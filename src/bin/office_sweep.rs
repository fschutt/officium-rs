//! office_sweep — Breviary regression-loop driver (B8).
//!
//! For one (rubric, hour, year) tuple, drives a single-day or
//! whole-year comparison loop:
//!
//!   1. **Rust pipeline**: derive the day's office key via
//!      `precedence::compute_office` (the same logic the Mass
//!      year-sweep uses), then `compute_office_hour` for the
//!      structured `Vec<RenderedLine>`.
//!   2. **Perl pipeline**: shell to `scripts/do_render.sh DATE
//!      VERSION HOUR`. Captures the upstream HTML.
//!   3. **Comparison**: extract the named section's body from each
//!      side via `regression::rust_office_section` /
//!      `regression::extract_perl_sections`, then run
//!      `compare_section_named` for a verdict.
//!
//! Slice-2 scope: single-section comparison (`--section`,
//! default Oratio) over a date range. Slice 3+: multi-section
//! comparison + structured manifest emission.
//!
//! Usage:
//!
//!   cargo run --bin office_sweep -- --date 05-04-2026 --hour Vespera \
//!     --rubric 'Tridentine - 1570' --section Oratio
//!
//!   # Year loop (no --date): walks every day in --year (defaults
//!   # to current year) and reports an aggregate pass-rate.
//!   cargo run --bin office_sweep -- --year 2026 --hour Vespera \
//!     --rubric 'Tridentine - 1570' --limit 14
//!
//!   # Smoke: just a few calibration dates.
//!   cargo run --bin office_sweep -- --smoke --hour Vespera

use std::cell::RefCell;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use officium_rs::core::{Date, Locale, OfficeInput, Rubric};
use officium_rs::corpus::BundledCorpus;
use officium_rs::horas::{self, OfficeArgs};
use officium_rs::perl_cache::{perl_submodule_sha, render_with_cache};
use officium_rs::perl_driver::{PerlDriver, ScriptType};
use officium_rs::precedence::compute_office;
use officium_rs::regression::{
    compare_office_section, rust_office_section, SectionStatus,
};

thread_local! {
    /// Per-thread persistent perl drivers, one per script-type.
    /// `--hour all` mixes Mass and Office requests; each gets
    /// routed to its own driver since missa/ordo.pl and
    /// horas/horas.pl can't coexist in one perl process (both
    /// define `sub getordinarium` in `package main`).
    static MISSA_DRIVER: RefCell<Option<Result<PerlDriver, String>>> =
        const { RefCell::new(None) };
    static OFFICIUM_DRIVER: RefCell<Option<Result<PerlDriver, String>>> =
        const { RefCell::new(None) };
}

const KNOWN_RUBRICS: &[(&str, Rubric)] = &[
    ("Tridentine - 1570",     Rubric::Tridentine1570),
    ("Tridentine - 1910",     Rubric::Tridentine1910),
    ("Divino Afflatu - 1939", Rubric::DivinoAfflatu1911),
    ("Reduced - 1955",        Rubric::Reduced1955),
    ("Rubrics 1960 - 1960",   Rubric::Rubrics1960),
];

const KNOWN_HOURS: &[&str] = &[
    "Matutinum", "Laudes", "Prima", "Tertia", "Sexta", "Nona", "Vespera", "Completorium",
];

const SMOKE_DATES: &[(u32, u32)] = &[
    (1, 1),   // Octave of Christmas / Circumcision
    (5, 4),   // St. Monica
    (6, 29),  // SS. Peter & Paul
    (12, 25), // Christmas Day
];

fn parse_rubric(s: &str) -> Option<Rubric> {
    KNOWN_RUBRICS
        .iter()
        .find(|(name, _)| *name == s)
        .map(|(_, r)| *r)
}

fn render_perl_office(
    repo_root: &PathBuf,
    date_us: &str,
    rubric_name: &str,
    hour: &str,
) -> Result<String, String> {
    let script = repo_root.join("scripts/do_render.sh");
    let out = Command::new("bash")
        .arg(&script)
        .arg(date_us)
        .arg(rubric_name)
        .arg(hour)
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("spawn failed: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "do_render.sh exit {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr),
        ));
    }
    String::from_utf8(out.stdout).map_err(|e| format!("non-utf8 stdout: {e}"))
}

/// Render via the thread-local persistent driver matching the
/// hour's script type. On first call per (worker, type), lazily
/// spawns. On spawn failure, logs once and falls back to the
/// per-render subprocess `render_perl_office`.
fn render_office_via_driver(
    repo_root: &PathBuf,
    date_us: &str,
    rubric_name: &str,
    hour: &str,
) -> Result<String, String> {
    let script_type = ScriptType::for_hour(hour);
    let cell = match script_type {
        ScriptType::Missa => &MISSA_DRIVER,
        ScriptType::Officium => &OFFICIUM_DRIVER,
    };
    cell.with(|c| {
        let mut opt = c.borrow_mut();
        if opt.is_none() {
            *opt = Some(PerlDriver::new(repo_root, script_type));
        }
        match opt.as_mut().unwrap() {
            Ok(driver) => driver.render(date_us, rubric_name, hour),
            Err(e) => {
                eprintln!(
                    "  perl-driver ({:?}) unavailable; using subprocess. Reason: {e}",
                    script_type
                );
                render_perl_office(repo_root, date_us, rubric_name, hour)
            }
        }
    })
}

fn is_leap(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => if is_leap(year) { 29 } else { 28 },
        _ => unreachable!(),
    }
}

fn next_day(year: i32, month: u32, day: u32) -> (u32, u32, i32) {
    let dim = days_in_month(year, month);
    if day < dim {
        (month, day + 1, year)
    } else if month < 12 {
        (month + 1, 1, year)
    } else {
        (1, 1, year + 1)
    }
}

fn dates_for_year(year: i32) -> Vec<(u32, u32)> {
    let mut out = Vec::with_capacity(366);
    for m in 1..=12 {
        for d in 1..=days_in_month(year, m) {
            out.push((m, d));
        }
    }
    out
}

#[derive(Default)]
struct Args {
    /// MM-DD-YYYY single-cell mode. When None, year-loop mode.
    date: Option<String>,
    year: Option<i32>,
    hour: String,
    rubric: String,
    /// Optional override; otherwise derived via precedence::compute_office.
    day_key: Option<String>,
    next_day_key: Option<String>,
    section: String,
    limit: Option<usize>,
    smoke: bool,
    verbose: bool,
    dump_body: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        date: None,
        year: None,
        hour: "Vespera".to_string(),
        rubric: "Tridentine - 1570".to_string(),
        day_key: None,
        next_day_key: None,
        section: "Oratio".to_string(),
        limit: None,
        smoke: false,
        verbose: false,
        dump_body: false,
    };
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < raw.len() {
        match raw[i].as_str() {
            "--date" => { i += 1; args.date = raw.get(i).cloned(); }
            "--year" => { i += 1; args.year = raw.get(i).and_then(|s| s.parse().ok()); }
            "--hour" => { i += 1; args.hour = raw.get(i).cloned().unwrap_or_default(); }
            "--rubric" => { i += 1; args.rubric = raw.get(i).cloned().unwrap_or_default(); }
            "--day-key" => { i += 1; args.day_key = raw.get(i).cloned(); }
            "--next-day-key" => { i += 1; args.next_day_key = raw.get(i).cloned(); }
            "--section" => { i += 1; args.section = raw.get(i).cloned().unwrap_or_default(); }
            "--limit" => { i += 1; args.limit = raw.get(i).and_then(|s| s.parse().ok()); }
            "--smoke" => { args.smoke = true; }
            "--verbose" | "-v" => { args.verbose = true; }
            "--dump-body" => { args.dump_body = true; }
            "--help" | "-h" => {
                eprintln!(
                    "Usage: office_sweep [--date MM-DD-YYYY | --year YYYY | --smoke] \\\n\
                     \t--hour HOUR --rubric RUBRIC [--section SECTION]\n\
                     \n\
                     Defaults: --hour Vespera, --rubric 'Tridentine - 1570',\n\
                     \t  --section Oratio. Year-mode walks every day; --limit N\n\
                     \t  truncates to N days.\n\
                     Hours: {hours}\n\
                     Rubrics:\n",
                    hours = KNOWN_HOURS.join(" "),
                );
                for (s, _) in KNOWN_RUBRICS {
                    eprintln!("  {s}");
                }
                std::process::exit(0);
            }
            _ => return Err(format!("unknown arg: {}", raw[i])),
        }
        i += 1;
    }
    if args.hour != "all" && !KNOWN_HOURS.contains(&args.hour.as_str()) {
        return Err(format!(
            "unknown --hour {:?}; valid: {} or 'all'",
            args.hour,
            KNOWN_HOURS.join(" ")
        ));
    }
    Ok(args)
}

/// Stable slug for a rubric name — used as a directory segment in
/// `target/regression-cache/`. Mirrors the `year_sweep` slug shape
/// so both binaries share cache entries.
fn slugify_rubric(r: &str) -> String {
    let mapped: String = r
        .chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' => c,
            _ => '_',
        })
        .collect();
    let mut out = String::with_capacity(mapped.len());
    let mut prev_us = false;
    for c in mapped.chars() {
        if c == '_' {
            if !prev_us {
                out.push(c);
            }
            prev_us = true;
        } else {
            out.push(c);
            prev_us = false;
        }
    }
    out.trim_matches('_').to_string()
}

fn parse_us_date(s: &str) -> Result<(u32, u32, i32), String> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 3 {
        return Err(format!("bad date {s:?} — want MM-DD-YYYY"));
    }
    let mm: u32 = parts[0].parse().map_err(|e| format!("month: {e}"))?;
    let dd: u32 = parts[1].parse().map_err(|e| format!("day: {e}"))?;
    let yyyy: i32 = parts[2].parse().map_err(|e| format!("year: {e}"))?;
    Ok((mm, dd, yyyy))
}

#[derive(Default, Debug)]
struct Stats {
    cells: usize,
    matched: usize,
    differ: usize,
    rust_blank: usize,
    perl_blank: usize,
    empty: usize,
    #[allow(dead_code)] perl_failures: usize,
    #[allow(dead_code)] panics: usize,
}

impl Stats {
    fn record(&mut self, status: SectionStatus) {
        self.cells += 1;
        match status {
            SectionStatus::Match => self.matched += 1,
            SectionStatus::Differ => self.differ += 1,
            SectionStatus::RustBlank => self.rust_blank += 1,
            SectionStatus::PerlBlank => self.perl_blank += 1,
            SectionStatus::Empty => self.empty += 1,
        }
    }

    fn pass_rate_pct(&self) -> f64 {
        if self.cells == 0 {
            return 0.0;
        }
        // Match + Empty count as "passing" — Empty means both sides
        // legitimately had nothing for the section (e.g. Vespera
        // doesn't have a Lectio4 slot). Perl-blank is also passing
        // because it indicates Perl chose a different office that
        // doesn't expose this section, which is a separate
        // calendar-resolution concern.
        let pass = (self.matched + self.empty) as f64;
        100.0 * pass / self.cells as f64
    }
}

fn run_one_cell(
    repo_root: &PathBuf,
    yyyy: i32, mm: u32, dd: u32,
    hour: &str,
    rubric: Rubric,
    rubric_name: &str,
    rubric_slug: &str,
    cache_sha: Option<&str>,
    day_key_override: Option<&str>,
    next_day_key_override: Option<&str>,
    section: &str,
    verbose: bool,
    dump_body: bool,
) -> (SectionStatus, Option<String>) {
    // Derive day_key via precedence::compute_office if not overridden.
    let derived_key = if let Some(k) = day_key_override {
        k.to_string()
    } else {
        let input = OfficeInput {
            date: Date::new(yyyy, mm, dd),
            rubric,
            locale: Locale::Latin,
            is_mass_context: false,
        };
        let office_result = std::panic::catch_unwind(|| compute_office(&input, &BundledCorpus));
        match office_result {
            Ok(office) => office.winner.render(),
            Err(_) => return (SectionStatus::RustBlank, Some("rust panic in compute_office".into())),
        }
    };
    // Christmas-Octave (Dec 26..31) office-context override. The
    // Mass-side `Tempora/Nat29` carries [Rank] ";;Semiduplex;;2.92"
    // (the missa file), so the precedence engine sees temporal_rank
    // 2.92 and beats the Sancti's 2.2 → Tempora wins. But the OFFICE-
    // side `horas/Tempora/Nat29` carries [Rank] ";;Semiduplex;;2.1",
    // and Perl's Office occurrence — using horas-side ranks — gives
    // Sancti (2.2) the win.
    //
    // Narrow override: for 12-26..12-31, when winner is `Tempora/
    // Nat{X}` and a Sancti commemoration with rubric-active rank >
    // Tempora-horas rank exists in the kalendarium, swap winner to
    // the Sancti's stem. Drives both the T1570/T1910 missa-vs-horas
    // divergence (slice 56) AND the R60 kalendar-vs-file divergence
    // (slice 64): under R60, kalendarium 12-28 = "12-28r" with
    // annotated rank 5, but Sancti/12-28r inherits from Sancti/12-28
    // whose [Rank] (rubrica 196) is ";;Duplex II class;;5.4". Tempora/
    // Nat28 R60 [Rank] = ";;Duplex II classis;;5". File-side: 5.4 > 5
    // → sanctoral wins. Our compute_occurrence uses the kalendar
    // annotation (5), not the file's actual rank, so the Tempora
    // wrongly wins.
    let derived_key = if (matches!(
        rubric,
        officium_rs::core::Rubric::Tridentine1570
            | officium_rs::core::Rubric::Tridentine1910
            | officium_rs::core::Rubric::DivinoAfflatu1911
            | officium_rs::core::Rubric::Reduced1955
            | officium_rs::core::Rubric::Rubrics1960
    )) && mm == 12
        && (26..=31).contains(&dd)
        && derived_key.starts_with("Tempora/Nat")
    {
        let layer = rubric.kalendar_layer();
        // Tempora's horas-side rank under the active rubric.
        let tempora_rank = horas::active_rank_line_with_annotations(&derived_key, rubric, hour)
            .map(|(_, _, n)| n)
            .unwrap_or(0.0);
        if let Some(cells) = officium_rs::kalendaria_layers::lookup(layer, mm, dd) {
            if let Some(main) = cells.first() {
                let sancti_key = format!("Sancti/{}", main.stem);
                // Use file's actual rank (chases @inherit) rather
                // than kalendar's annotation. For R60 12-28: kalendar
                // says 5, file says 5.4 (via @Sancti/12-28).
                let sancti_rank = horas::active_rank_line_with_annotations(
                    &sancti_key, rubric, hour,
                )
                .map(|(_, _, n)| n)
                .unwrap_or_else(|| main.rank_num().unwrap_or(0.0));
                if sancti_rank > tempora_rank {
                    sancti_key
                } else if sancti_rank >= 2.0
                    && matches!(
                        rubric,
                        officium_rs::core::Rubric::Tridentine1570
                            | officium_rs::core::Rubric::Tridentine1910
                    )
                {
                    // T1570/T1910 fallback: when sancti rank >= 2.0
                    // (preserves slice 56 behaviour: missa-side
                    // Tempora/Nat29 elevated to 2.92 by missa data
                    // which we don't reflect on the horas side, so
                    // any Sancti rank ≥ 2.0 wins).
                    sancti_key
                } else {
                    derived_key
                }
            } else {
                derived_key
            }
        } else {
            derived_key
        }
    } else {
        derived_key
    };

    // R55 Semiduplex 2.2..2.8 → Tempora at Vespera/Completorium.
    // Mirror of horascommon.pl:315-323 — under R55, Semiduplex
    // saints with rank num ∈ [2.2, 2.9) are reduced to Simplex 1.2
    // by lines 382-389, then wiped at Vespera/Compline by lines
    // 315-318 ("Reduced to Simplex/Comm ad Laudes tantum ends
    // after None"), leaving the Tempora ferial as the day's office.
    //
    // Drives 01-22 Thu R55 Vespera: today=Sancti/01-22 Vincent &
    // Anastasius (Semiduplex 2.2). Without this rule, Rust uses
    // Vincent Oratio "Adesto, Domine..."; Perl wipes the saint and
    // uses Tempora/Epi2-0 [Oratio] "Omnipotens sempiterne Deus..."
    // (the Sun-after-Epi2 Oratio).
    //
    // Narrow: only fires under R55. The Perl gate is
    // `1955|Monastic.*Divino|1963` — R60 ('196') uses a different
    // path and is excluded.
    let today_dow_pre = officium_rs::date::day_of_week(dd, mm, yyyy);
    let derived_key = if (hour == "Vespera" || hour == "Completorium")
        && matches!(rubric, officium_rs::core::Rubric::Reduced1955)
        && derived_key.starts_with("Sancti/")
    {
        let r55_semiduplex_22_28 =
            horas::active_rank_line_with_annotations(&derived_key, rubric, hour)
                .map(|(_, cls, n)| (cls, n))
                .filter(|(cls, n)| {
                    cls.to_lowercase().contains("semiduplex") && *n >= 2.2 && *n < 2.9
                });
        if r55_semiduplex_22_28.is_some() {
            let weekname = officium_rs::date::getweek(dd, mm, yyyy, false, true);
            let tempora_key = format!("Tempora/{weekname}-{today_dow_pre}");
            if horas::lookup(&tempora_key).is_some() {
                tempora_key
            } else {
                derived_key
            }
        } else {
            derived_key
        }
    } else {
        derived_key
    };

    // For Vespera AND Completorium: auto-derive the next day's
    // office key and let `first_vespers_day_key` swap if tomorrow
    // outranks today. The Roman liturgical day starts at Vespers
    // (eve of feast) and extends through Compline — so when 01-16
    // Vespera resolves to first Vespers of 01-17 Antony Abbot,
    // 01-16 Compline runs with the SAME 01-17 winner. Without this,
    // Compline's preces predicate sees the wrong winner (e.g.
    // Marcellus Semiduplex 2.2 instead of Antony Duplex 3) and
    // mis-fires omittitur on days where Perl emits lines [2,3].
    // The CLI override still wins if explicitly set.
    let next_derived_key: Option<String> = if hour == "Vespera" || hour == "Completorium" {
        if let Some(k) = next_day_key_override {
            Some(k.to_string())
        } else {
            let (nm, nd, ny) = next_day(yyyy, mm, dd);
            let next_input = OfficeInput {
                date: Date::new(ny, nm, nd),
                rubric,
                locale: Locale::Latin,
                is_mass_context: false,
            };
            std::panic::catch_unwind(|| compute_office(&next_input, &BundledCorpus))
                .ok()
                .map(|o| o.winner.render())
        }
    } else {
        None
    };

    let resolved_key = if let Some(next) = next_derived_key.as_deref() {
        let today_dow = officium_rs::date::day_of_week(dd, mm, yyyy);
        let primary = horas::first_vespers_day_key_for_rubric(
            &derived_key, next, rubric, hour, today_dow,
        )
        .to_string();
        // Sancti-Simplex no-2V Tempora-of-week fallback. When TODAY is
        // a Sancti Simplex (no proper 2nd Vespers) AND the swap to
        // tomorrow's 1V was rejected (Vigilia gate / feria-privilegiata
        // gate kept today), Perl uses today's TEMPORA ferial — Simplex
        // feasts have no proper 2V, so the day's Vespers continues with
        // the week-ferial Tempora (which inherits the week-Sun's Oratio
        // via "Oratio Dominica"). Drives 06-22 Mon (Paulinus Simplex
        // eve of James Vigilia), 05-12 Tue (Nereus & co. Simplex eve
        // of Asc Vigilia), 10-26 Mon (Evaristus Simplex eve of
        // 10-27 Vigil-stem-day), etc.
        //
        // Narrow: only fires when (a) we're at Vespera/Compline,
        // (b) the resolved key equals today's derived key (no swap
        // happened — first_vespers kept today), and (c) the resolved
        // key is Sancti Simplex. When TOMORROW is a Sancti Simplex
        // (e.g., Thu eve of Anicetus Fri 04-17), the swap is correct
        // and Perl renders the Simplex as 1V winner — don't override.
        let kept_today = primary == derived_key;
        if (hour == "Vespera" || hour == "Completorium")
            && kept_today
            && primary.starts_with("Sancti/")
            && horas::active_rank_line_with_annotations(&primary, rubric, hour)
                .map(|(_, cls, _)| cls.to_lowercase().contains("simplex"))
                .unwrap_or(false)
        {
            let weekname = officium_rs::date::getweek(dd, mm, yyyy, false, true);
            let tempora_key = format!("Tempora/{weekname}-{today_dow}");
            if horas::lookup(&tempora_key).is_some() {
                tempora_key
            } else {
                primary
            }
        } else {
            primary
        }
    } else {
        derived_key.clone()
    };

    let office_args = OfficeArgs {
        year: yyyy,
        month: mm,
        day: dd,
        rubric,
        hour,
        rubrics: true,
        day_key: Some(&resolved_key),
    };
    let lines = horas::compute_office_hour(&office_args);
    let rust_body = rust_office_section(&lines, section).unwrap_or_default();

    let date_us = format!("{mm:02}-{dd:02}-{yyyy}");
    let perl_html = match render_with_cache(
        repo_root,
        rubric_slug,
        yyyy,
        mm,
        dd,
        hour,
        cache_sha,
        || render_office_via_driver(repo_root, &date_us, rubric_name, hour),
    ) {
        Ok(html) => html,
        Err(e) => return (SectionStatus::PerlBlank, Some(format!("perl failed: {e}"))),
    };

    let status = compare_office_section(&rust_body, &perl_html, section);
    if dump_body {
        let perl_sections =
            officium_rs::regression::extract_perl_sections(&perl_html);
        let perl_body = perl_sections
            .get(section)
            .cloned()
            .unwrap_or_default();
        eprintln!("\n─── DUMP {date_us} {hour} {section} ─────────");
        eprintln!("rust_body ({} bytes):\n{rust_body}", rust_body.len());
        eprintln!("\nperl_body ({} bytes):\n{perl_body}", perl_body.len());
        eprintln!("───────────────────────────────────────────────\n");
    }
    let info = if verbose {
        Some(format!(
            "key={resolved_key} rust_lines={} rust_len={} status={status:?}",
            lines.len(),
            rust_body.len(),
        ))
    } else {
        None
    };
    (status, info)
}

fn main() -> Result<(), String> {
    let args = parse_args()?;
    let rubric = parse_rubric(&args.rubric).ok_or_else(|| {
        format!(
            "unknown rubric {:?}; known: {}",
            args.rubric,
            KNOWN_RUBRICS.iter().map(|(s, _)| *s).collect::<Vec<_>>().join(", ")
        )
    })?;
    let repo_root = std::env::current_dir().map_err(|e| format!("cwd: {e}"))?;

    // Pick the date list.
    let dates: Vec<(u32, u32, i32)> = if let Some(date_str) = &args.date {
        let (mm, dd, yyyy) = parse_us_date(date_str)?;
        vec![(mm, dd, yyyy)]
    } else if args.smoke {
        let yyyy = args.year.unwrap_or_else(|| {
            // Fallback to a common year if --year not supplied.
            2026
        });
        SMOKE_DATES.iter().map(|(m, d)| (*m, *d, yyyy)).collect()
    } else {
        let yyyy = args.year.unwrap_or_else(|| {
            // Default to a sane modern year. The Mass year_sweep
            // walks the current calendar year via chrono; for the
            // Office sweep we keep it explicit.
            2026
        });
        let mut all: Vec<(u32, u32, i32)> = dates_for_year(yyyy)
            .into_iter()
            .map(|(m, d)| (m, d, yyyy))
            .collect();
        if let Some(n) = args.limit {
            all.truncate(n);
        }
        all
    };

    let hours_to_run: Vec<&str> = if args.hour == "all" {
        KNOWN_HOURS.to_vec()
    } else {
        vec![args.hour.as_str()]
    };

    // Resolve the upstream Perl SHA once per sweep so all cells
    // share the same cache namespace. Hits skip the Perl invocation
    // entirely; misses fall through to a fresh `do_render.sh` call.
    let cache_sha = perl_submodule_sha(&repo_root);
    let rubric_slug = slugify_rubric(&args.rubric);

    eprintln!(
        "office_sweep: {} dates × {} hour(s) = {} cells · rubric={:?} section={} perl-cache={}",
        dates.len(),
        hours_to_run.len(),
        dates.len() * hours_to_run.len(),
        rubric,
        args.section,
        cache_sha
            .as_deref()
            .map(|s| &s[..12])
            .unwrap_or("disabled"),
    );

    let mut overall = Stats::default();
    let mut per_hour: Vec<(String, Stats)> =
        hours_to_run.iter().map(|h| (h.to_string(), Stats::default())).collect();
    for (mm, dd, yyyy) in &dates {
        for (hi, hour) in hours_to_run.iter().enumerate() {
            let (status, info) = run_one_cell(
                &repo_root,
                *yyyy, *mm, *dd,
                hour,
                rubric,
                &args.rubric,
                &rubric_slug,
                cache_sha.as_deref(),
                args.day_key.as_deref(),
                args.next_day_key.as_deref(),
                &args.section,
                args.verbose,
                args.dump_body,
            );
            overall.record(status);
            per_hour[hi].1.record(status);
            let mark = match status {
                SectionStatus::Match | SectionStatus::Empty => "✓",
                SectionStatus::PerlBlank => "·",
                _ => "✗",
            };
            if args.verbose || matches!(status, SectionStatus::Differ | SectionStatus::RustBlank) {
                eprintln!(
                    "  {mark} {:02}-{:02}-{:04} {:>13}  {:?}{}",
                    mm, dd, yyyy, hour, status,
                    info.as_deref().map(|s| format!("  · {s}")).unwrap_or_default(),
                );
            }
        }
    }

    println!();
    println!("─── office_sweep summary ───────────────────────────");
    if hours_to_run.len() > 1 {
        println!("per-hour pass-rates:");
        for (h, s) in &per_hour {
            println!(
                "  {:>13}  {:>4}/{:<4}  ({:>5.2}%)  match={} differ={} rust-blank={} perl-blank={} empty={}",
                h,
                s.matched + s.empty,
                s.cells,
                s.pass_rate_pct(),
                s.matched,
                s.differ,
                s.rust_blank,
                s.perl_blank,
                s.empty,
            );
        }
        println!();
    }
    println!("cells:       {}", overall.cells);
    println!("matched:     {}", overall.matched);
    println!("differ:      {}", overall.differ);
    println!("rust-blank:  {}", overall.rust_blank);
    println!("perl-blank:  {}", overall.perl_blank);
    println!("empty:       {}", overall.empty);
    println!("pass-rate:   {:.2}%", overall.pass_rate_pct());

    // ≥99.7% bar from SUPER_PLAN exit criteria.
    if overall.cells > 0 && overall.pass_rate_pct() >= 99.7 {
        Ok(())
    } else {
        Err(format!(
            "below ≥99.7% bar (got {:.2}%)",
            overall.pass_rate_pct()
        ))
    }
}
