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

use std::path::PathBuf;
use std::process::{Command, Stdio};

use officium_rs::core::{Date, Locale, OfficeInput, Rubric};
use officium_rs::corpus::BundledCorpus;
use officium_rs::horas::{self, OfficeArgs};
use officium_rs::precedence::compute_office;
use officium_rs::regression::{
    compare_office_section, rust_office_section, SectionStatus,
};

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
    if !KNOWN_HOURS.contains(&args.hour.as_str()) {
        return Err(format!(
            "unknown --hour {:?}; valid: {}",
            args.hour,
            KNOWN_HOURS.join(" ")
        ));
    }
    Ok(args)
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
    day_key_override: Option<&str>,
    next_day_key_override: Option<&str>,
    section: &str,
    verbose: bool,
) -> (SectionStatus, Option<String>) {
    // Derive day_key via precedence::compute_office if not overridden.
    let derived_key = if let Some(k) = day_key_override {
        k.to_string()
    } else {
        let input = OfficeInput {
            date: Date::new(yyyy, mm, dd),
            rubric,
            locale: Locale::Latin,
        };
        let office_result = std::panic::catch_unwind(|| compute_office(&input, &BundledCorpus));
        match office_result {
            Ok(office) => office.winner.render(),
            Err(_) => return (SectionStatus::RustBlank, Some("rust panic in compute_office".into())),
        }
    };

    let resolved_key = if hour == "Vespera" {
        if let Some(next) = next_day_key_override {
            horas::first_vespers_day_key(&derived_key, next).to_string()
        } else {
            derived_key.clone()
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
    let perl_html = match render_perl_office(repo_root, &date_us, rubric_name, hour) {
        Ok(html) => html,
        Err(e) => return (SectionStatus::PerlBlank, Some(format!("perl failed: {e}"))),
    };

    let status = compare_office_section(&rust_body, &perl_html, section);
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

    eprintln!(
        "office_sweep: {} cells · hour={} rubric={:?} section={}",
        dates.len(),
        args.hour,
        rubric,
        args.section,
    );

    let mut stats = Stats::default();
    for (mm, dd, yyyy) in &dates {
        let (status, info) = run_one_cell(
            &repo_root,
            *yyyy, *mm, *dd,
            &args.hour,
            rubric,
            &args.rubric,
            args.day_key.as_deref(),
            args.next_day_key.as_deref(),
            &args.section,
            args.verbose,
        );
        stats.record(status);
        let mark = match status {
            SectionStatus::Match | SectionStatus::Empty => "✓",
            SectionStatus::PerlBlank => "·",
            _ => "✗",
        };
        if args.verbose || matches!(status, SectionStatus::Differ | SectionStatus::RustBlank) {
            eprintln!(
                "  {mark} {:02}-{:02}-{:04}  {:?}{}",
                mm, dd, yyyy, status,
                info.as_deref().map(|s| format!("  · {s}")).unwrap_or_default(),
            );
        }
    }

    println!();
    println!("─── office_sweep summary ───────────────────────────");
    println!("cells:       {}", stats.cells);
    println!("matched:     {}", stats.matched);
    println!("differ:      {}", stats.differ);
    println!("rust-blank:  {}", stats.rust_blank);
    println!("perl-blank:  {}", stats.perl_blank);
    println!("empty:       {}", stats.empty);
    println!("pass-rate:   {:.2}%", stats.pass_rate_pct());

    // ≥99.7% bar from SUPER_PLAN exit criteria.
    if stats.cells > 0 && stats.pass_rate_pct() >= 99.7 {
        Ok(())
    } else {
        Err(format!(
            "below ≥99.7% bar (got {:.2}%)",
            stats.pass_rate_pct()
        ))
    }
}
