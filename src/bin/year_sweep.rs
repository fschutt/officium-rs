//! year_sweep — Phase 6 Rust↔Perl regression-loop driver.
//!
//! For every date in `--year` (default current year):
//!
//!   1. **Rust pipeline**: build `OfficeInput` → `compute_office` →
//!      `mass_propers`. Captures `office.winner` plus the resolved
//!      Latin propers.
//!   2. **Perl pipeline**: shell to `scripts/do_render.sh DATE VERSION
//!      SanctaMissa`. Captures the upstream HTML.
//!   3. **Comparison**: `regression::compare_day()` extracts each
//!      Latin section from the Perl HTML and asserts the Rust
//!      proper appears as a substring (modulo diacritic / punctuation
//!      / whitespace normalisation).
//!
//! Output:
//!
//!   target/regression/{slug}-{year}/{MM-DD}.perl.html      raw cache
//!   target/regression/{slug}-{year}/manifest.json          aggregate
//!   target/regression/{slug}-{year}/board.html             grid
//!
//! Usage:
//!
//!   cargo run --bin year-sweep                                full current year, Tridentine 1570
//!   cargo run --bin year-sweep -- --year 2026                 explicit year
//!   cargo run --bin year-sweep -- --year 2026 --limit 14      first N dates
//!   cargo run --bin year-sweep -- --smoke                     3 calibration dates
//!   cargo run --bin year-sweep -- --rubric 'Rubrics 1960 - 1960'
//!     (panics until Phase 7-10 reform layers ship)
//!
//! See DIVINUM_OFFICIUM_PORT_PLAN.md Phase 6.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Instant;

use rayon::prelude::*;

use officium_rs::core::{Date, Locale, MassPropers, OfficeInput, Rubric};
use officium_rs::corpus::BundledCorpus;
use officium_rs::mass::mass_propers;
use officium_rs::perl_cache::{perl_submodule_sha, render_with_cache};
use officium_rs::perl_driver::{PerlDriver, ScriptType};
use officium_rs::precedence::compute_office;
use officium_rs::regression::{
    compare_day, explain_divergence, extract_perl_sections, infer_perl_source, normalize,
    strip_perl_rubrics, DayReport, DivergenceCategory, InferredSource, SectionStatus,
    PROPER_SECTIONS,
};

thread_local! {
    /// Per-thread Perl driver. `None` = not yet initialised;
    /// `Some(Ok(d))` = up and running; `Some(Err(e))` = init
    /// failed, fall back to per-render subprocess for this worker.
    /// Each rayon worker thread holds one driver for the lifetime
    /// of the sweep, eliminating per-render perl-startup cost.
    static PERL_DRIVER: RefCell<Option<Result<PerlDriver, String>>> = const { RefCell::new(None) };
}

// Perl missa.pl `check_version` accepts only the names declared in
// `web/www/missa/missa.dialog [versions]`. Bare "Divino Afflatu" is
// ambiguous (matches both -1939 and -1954) and falls back to
// "Rubrics 1960 - 1960" with an `Unknown version` error — silently
// poisoning every DA regression run. The DA-1939 form is the one our
// `Rubric::DivinoAfflatu1911` resolves to (kalendar 1939, transfer DA).
//
// Monastic missa has no dedicated rubric — Perl routes it to Tridentine
// 1570 — so we omit it here; running it would just duplicate that sweep.
const KNOWN_RUBRICS: &[(&str, Rubric)] = &[
    ("Tridentine - 1570",     Rubric::Tridentine1570),
    ("Tridentine - 1910",     Rubric::Tridentine1910),
    ("Divino Afflatu - 1939", Rubric::DivinoAfflatu1911),
    ("Reduced - 1955",        Rubric::Reduced1955),
    ("Rubrics 1960 - 1960",   Rubric::Rubrics1960),
];

fn usage() -> ! {
    eprintln!(
        "Usage: year-sweep [--year YYYY] [--rubric NAME] [--limit N] [--smoke] [--out DIR]\n\
         \n\
         Defaults: --year (current year), --rubric 'Tridentine - 1570'.\n\
         \n\
         Known rubrics:"
    );
    for (s, _) in KNOWN_RUBRICS {
        eprintln!("    {s:?}");
    }
    std::process::exit(2);
}

fn repo_root() -> PathBuf {
    // `vendor/divinum-officium/` and `scripts/` live at the crate
    // root in officium-rs (formerly one level up in the website
    // monorepo). Drop the historic `pop()` and use the manifest dir
    // directly.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn assert_vendor_present(root: &PathBuf) {
    let missa_pl = root.join("vendor/divinum-officium/web/cgi-bin/missa/missa.pl");
    if !missa_pl.exists() {
        eprintln!(
            "FATAL: vendor/divinum-officium/ missing (expected {}).\n\
             Run: bash scripts/setup-divinum-officium.sh",
            missa_pl.display()
        );
        std::process::exit(1);
    }
}

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

fn current_year() -> i32 {
    // Days since 1970-01-01, then forward-walk year by year. Cheap
    // and avoids depending on chrono. Approximate (no leap-second
    // care) but accurate to the current calendar year.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let mut days = secs / 86_400;
    let mut year = 1970i32;
    loop {
        let yd = if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
            366
        } else {
            365
        };
        if days < yd {
            return year;
        }
        days -= yd;
        year += 1;
    }
}

fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
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

fn render_perl(root: &PathBuf, mm: u32, dd: u32, yyyy: i32, rubric: &str) -> Result<String, String> {
    let date = format!("{mm:02}-{dd:02}-{yyyy}");
    let script = root.join("scripts/do_render.sh");
    let out = Command::new("bash")
        .arg(&script)
        .arg(&date)
        .arg(rubric)
        .arg("SanctaMissa")
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("spawn failed: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "do_render.sh exit {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    String::from_utf8(out.stdout).map_err(|e| format!("non-utf8 stdout: {e}"))
}

/// Render via the thread-local persistent driver. On first call
/// per worker thread, lazily spawns a `PerlDriver`. If spawn fails
/// (e.g. system perl missing CGI module), falls back to the
/// per-render subprocess `render_perl` and remembers the failure
/// so we don't retry the spawn for every cell.
///
/// `subprocess_fallback_logged` is shared across all workers so
/// the subprocess-fallback warning prints exactly once per sweep,
/// not once per worker.
fn render_via_driver(
    root: &PathBuf,
    mm: u32,
    dd: u32,
    yyyy: i32,
    rubric: &str,
    subprocess_fallback_logged: &AtomicBool,
) -> Result<String, String> {
    let date = format!("{mm:02}-{dd:02}-{yyyy}");
    PERL_DRIVER.with(|cell| {
        let mut opt = cell.borrow_mut();
        if opt.is_none() {
            // year_sweep only ever asks for SanctaMissa, so the
            // missa-only driver is the right one. office_sweep
            // uses both types and routes per request.
            *opt = Some(PerlDriver::new(root, ScriptType::Missa));
        }
        match opt.as_mut().unwrap() {
            Ok(driver) => driver.render(&date, rubric, "SanctaMissa"),
            Err(e) => {
                if !subprocess_fallback_logged.swap(true, Ordering::Relaxed) {
                    eprintln!(
                        "  perl-driver unavailable; falling back to subprocess. \
                         Reason: {e}"
                    );
                }
                render_perl(root, mm, dd, yyyy, rubric)
            }
        }
    })
}

/// Per-day outcome carried back from a rayon worker. Carries the
/// payloads needed for the sequential aggregation pass — the report
/// + winner string for stats, plus the inferred-source extracts
/// (Perl sections + winner headline) needed for the
/// `inferred_misses`/`inferred_pairs` BTreeMap merges. The raw HTML
/// is written to the cache file inside the worker so we don't drag
/// 365× a few-KB strings through the aggregator.
struct DayResult {
    mm: u32,
    dd: u32,
    elapsed_ms: u128,
    payload: DayPayload,
}

enum DayPayload {
    Done {
        rust_winner: String,
        report: DayReport,
        /// Pre-extracted Perl sections (only the sections we need
        /// for the inferred-source aggregation), to avoid re-parsing
        /// the HTML in the sequential pass.
        perl_sections: BTreeMap<String, String>,
    },
    Panic(String),
    PerlFail(String),
}

#[derive(Default)]
struct Stats {
    days_total: usize,
    days_passing: usize,
    days_winner_match: usize,
    section_match: usize,
    section_differ: usize,
    section_rust_blank: usize,
    section_perl_blank: usize,
    section_empty: usize,
    /// Differ-cell breakdown by divergence category.
    cat_macro_not_expanded: usize,
    cat_rubric_injection: usize,
    cat_ortho_variant: usize,
    cat_trailing_extra: usize,
    cat_leading_extra: usize,
    cat_other: usize,
    /// Per-section pass rates (Match + Empty / total).
    per_section_pass: [usize; 12],
    per_section_total: [usize; 12],
    panics: Vec<(u32, u32, String)>,
    perl_failures: Vec<(u32, u32, String)>,
    /// Aggregate "what file did Perl actually use, where Rust didn't"
    /// counts. Key = `<file>:<section>`. Captures the Phase 7+ work
    /// list: every entry is a Tempora/Sancti/Commune file that the
    /// Rust pipeline isn't currently selecting.
    inferred_misses: BTreeMap<String, usize>,
    /// Aggregate "rust file → perl file" pair counts. Key =
    /// `<rust_file> -> <perl_file>`. Surfaces systematic 1570→1962
    /// kalendar diffs (e.g. `Tempora/Epi1-0 -> Tempora/Epi1-0a`
    /// repeated across the Octave of Epiphany).
    inferred_pairs: BTreeMap<String, usize>,
}

struct Cfg {
    year: i32,
    /// Optional inclusive year range — when set, the binary loops
    /// over each year FROM..=TO and emits per-year output dirs.
    /// Used by the CI 100-year regression workflow.
    years: Option<(i32, i32)>,
    rubric_str: String,
    rubric: Rubric,
    limit: Option<usize>,
    smoke: bool,
    out_override: Option<PathBuf>,
    /// When true, emit per-day `MM-DD.diff.md` files showing each
    /// section's rust raw / rust normalized / perl raw (excerpt) /
    /// perl normalized + first-divergence context.
    dump: bool,
    /// When true (default), the rolling progress line prints the
    /// current section-pass rate so we get live feedback.
    progress: bool,
    /// When true, exit with non-zero status if any section diverges.
    /// CI uses this to fail the workflow on regressions.
    strict: bool,
}

fn parse_args() -> Cfg {
    let mut year: Option<i32> = None;
    let mut years: Option<(i32, i32)> = None;
    let mut rubric_str = String::from("Tridentine - 1570");
    let mut limit: Option<usize> = None;
    let mut smoke = false;
    let mut out_override: Option<PathBuf> = None;
    let mut dump = false;
    let mut progress = true;
    let mut strict = false;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--year" => {
                i += 1;
                year = Some(args.get(i).and_then(|s| s.parse().ok()).unwrap_or_else(|| usage()));
            }
            "--years" => {
                // Accept `1976:2076` or `1976..=2076` or `1976..2077`.
                i += 1;
                let s = args.get(i).cloned().unwrap_or_else(|| usage());
                let parsed = parse_year_range(&s).unwrap_or_else(|| {
                    eprintln!("unknown --years value {s:?}; expected FROM:TO");
                    usage();
                });
                years = Some(parsed);
            }
            "--rubric" => {
                i += 1;
                rubric_str = args.get(i).cloned().unwrap_or_else(|| usage());
            }
            "--limit" => {
                i += 1;
                limit = Some(args.get(i).and_then(|s| s.parse().ok()).unwrap_or_else(|| usage()));
            }
            "--out" => {
                i += 1;
                out_override = Some(PathBuf::from(args.get(i).cloned().unwrap_or_else(|| usage())));
            }
            "--smoke" => smoke = true,
            "--dump" => dump = true,
            "--quiet" => progress = false,
            "--strict" => strict = true,
            "-h" | "--help" => usage(),
            other => {
                eprintln!("unknown arg: {other}");
                usage();
            }
        }
        i += 1;
    }

    let rubric = KNOWN_RUBRICS
        .iter()
        .find(|(s, _)| *s == rubric_str)
        .map(|(_, r)| *r)
        .unwrap_or_else(|| {
            eprintln!("unknown rubric: {rubric_str:?}");
            usage();
        });

    Cfg {
        year: year.unwrap_or_else(current_year),
        years,
        rubric_str,
        rubric,
        limit,
        smoke,
        out_override,
        dump,
        progress,
        strict,
    }
}

fn parse_year_range(s: &str) -> Option<(i32, i32)> {
    // Try `FROM:TO` first (cleanest for shells).
    if let Some((a, b)) = s.split_once(':') {
        return Some((a.parse().ok()?, b.parse().ok()?));
    }
    // `..=` and `..` Rust-range syntax for convenience.
    if let Some((a, b)) = s.split_once("..=") {
        return Some((a.parse().ok()?, b.parse().ok()?));
    }
    if let Some((a, b)) = s.split_once("..") {
        let to: i32 = b.parse().ok()?;
        return Some((a.parse().ok()?, to - 1));
    }
    None
}

fn main() {
    let cfg = parse_args();
    let root = repo_root();
    assert_vendor_present(&root);

    if let Some((from, to)) = cfg.years {
        let mut any_failed = false;
        let mut total_pass = 0u32;
        let mut total_total = 0u32;
        for year in from..=to {
            let (pass, total, ok) = run_one_year(&cfg, year, &root);
            total_pass += pass;
            total_total += total;
            if !ok {
                any_failed = true;
            }
        }
        let pct = if total_total > 0 {
            total_pass as f64 / total_total as f64 * 100.0
        } else {
            0.0
        };
        println!();
        println!("=== multi-year summary ===");
        println!(
            "  rubric:      {}",
            cfg.rubric_str,
        );
        println!(
            "  years:       {}..={} ({} years)",
            from,
            to,
            to - from + 1,
        );
        println!(
            "  days passing: {}/{} ({:.2}%)",
            total_pass, total_total, pct,
        );
        if cfg.strict && (any_failed || total_pass < total_total) {
            std::process::exit(1);
        }
        return;
    }

    let (_, _, ok) = run_one_year(&cfg, cfg.year, &root);
    if cfg.strict && !ok {
        std::process::exit(1);
    }
}

/// Run the year-sweep for a single year. Returns
/// `(days_passing, days_total, no_panics_or_perl_failures)` so the
/// caller can aggregate across a range.
fn run_one_year(cfg: &Cfg, year: i32, root: &PathBuf) -> (u32, u32, bool) {
    let dates: Vec<(u32, u32)> = if cfg.smoke {
        vec![(1, 1), (4, 30), (12, 25)]
    } else {
        let mut d = dates_for_year(year);
        if let Some(n) = cfg.limit {
            d.truncate(n);
        }
        d
    };

    let slug = slugify_rubric(&cfg.rubric_str);
    let out_dir = cfg
        .out_override
        .clone()
        .unwrap_or_else(|| root.join(format!("target/regression/{slug}-{}", year)));
    fs::create_dir_all(&out_dir).expect("create output dir");

    let total = dates.len();
    let started = Instant::now();
    let mut stats = Stats::default();
    let mut day_reports: Vec<DayReport> = Vec::with_capacity(total);

    // Resolve the upstream Perl SHA once per year-sweep so workers
    // share a single cache namespace. `None` disables caching (e.g.
    // when run outside a git tree).
    let cache_sha = perl_submodule_sha(root);

    // Logged-once-per-sweep flag for the subprocess-fallback path.
    // If the persistent driver fails to spawn on a worker thread,
    // we want exactly ONE diagnostic line in the sweep log, not
    // 365 of them.
    let subprocess_fallback_logged = AtomicBool::new(false);

    println!(
        "year-sweep: rubric={:?} year={} dates={} out={} perl-cache={}",
        cfg.rubric_str,
        year,
        total,
        out_dir.display(),
        cache_sha
            .as_deref()
            .map(|s| &s[..12])
            .unwrap_or("disabled")
    );

    // ── Parallel per-day work. Each rayon worker runs the
    // (Rust-render → Perl-render → compare → cache writes) pipeline
    // for one date. The HTML cache and `--dump` diff write happen
    // inside the worker so we don't carry the raw HTML through
    // aggregation memory. The aggregator then walks the returned
    // `DayResult`s sequentially to merge stats and BTreeMap counts.
    //
    // Thread-count: defaults to the rayon global pool (physical
    // cores). CI runners are 4-core, so this gives a ~4× wall-clock
    // win for the Perl-bound matrix at zero correctness risk —
    // the Rust pipeline is pure (BundledCorpus is OnceLock-backed)
    // and Perl is a fresh subprocess per call.
    let done_counter = AtomicUsize::new(0);
    let mut day_results: Vec<DayResult> = dates
        .par_iter()
        .map(|(mm, dd)| {
            let t0 = Instant::now();
            let date_label = format!("{:04}-{:02}-{:02}", year, mm, dd);

            // Rust pipeline (catch panics so one bad day doesn't
            // abort the whole sweep).
            let input = OfficeInput {
                date: Date::new(year, *mm, *dd),
                rubric: cfg.rubric,
                locale: Locale::Latin,
                is_mass_context: true,
            };
            let rust_result = std::panic::catch_unwind(|| {
                let office = compute_office(&input, &BundledCorpus);
                let propers = mass_propers(&office, &BundledCorpus);
                (office.winner.render(), propers)
            });
            let (rust_winner, rust_propers) = match rust_result {
                Ok(p) => p,
                Err(e) => {
                    let msg = panic_message(&e);
                    return DayResult {
                        mm: *mm,
                        dd: *dd,
                        elapsed_ms: t0.elapsed().as_millis(),
                        payload: DayPayload::Panic(msg),
                    };
                }
            };

            // Perl pipeline (cache-first; on miss, route through
            // the thread-local persistent driver instead of a fresh
            // subprocess for every render).
            let perl_html = match render_with_cache(
                &root,
                &slug,
                year,
                *mm,
                *dd,
                "SanctaMissa",
                cache_sha.as_deref(),
                || {
                    render_via_driver(
                        &root,
                        *mm,
                        *dd,
                        year,
                        &cfg.rubric_str,
                        &subprocess_fallback_logged,
                    )
                },
            ) {
                Ok(h) => h,
                Err(e) => {
                    return DayResult {
                        mm: *mm,
                        dd: *dd,
                        elapsed_ms: t0.elapsed().as_millis(),
                        payload: DayPayload::PerlFail(e),
                    };
                }
            };
            // Per-day raw-HTML cache USED to live at
            // `out_dir/<MM-DD>.perl.html` for human inspection.
            // Dropped — `target/regression-cache/<sha>/<rubric>/<YYYY>/`
            // (the SHA-keyed cache wired by `render_with_cache`)
            // already holds the same content under a deterministic
            // path, and double-storing 365×100×5 = 184k HTML files
            // burns ~17 GB. The board.html + manifest.json still
            // get the per-day data they need from the in-memory
            // `perl_html` we already have here.

            // Compare.
            let report = compare_day(&date_label, &rust_winner, &rust_propers, &perl_html);
            let perl_sections = extract_perl_sections(&perl_html);

            if cfg.dump {
                let dump_path = out_dir.join(format!("{:02}-{:02}.diff.md", mm, dd));
                let dump_text = render_day_diff(
                    &date_label,
                    &rust_winner,
                    &rust_propers,
                    &perl_html,
                    &report,
                );
                let _ = fs::write(&dump_path, dump_text);
            }

            // Live progress: a counter incremented on completion. We
            // only print every 25 done dates to avoid contention; the
            // exact ordinal a worker prints isn't deterministic but
            // total-count and elapsed-ms both are, which is what the
            // user cares about.
            let i = done_counter.fetch_add(1, Ordering::Relaxed) + 1;
            if cfg.progress && (i % 25 == 0 || i == total) {
                println!(
                    "  [{:>3}/{:>3}] {:5.1}% {:02}-{:02}  ({} ms)",
                    i,
                    total,
                    i as f32 / total as f32 * 100.0,
                    mm,
                    dd,
                    t0.elapsed().as_millis(),
                );
            }

            DayResult {
                mm: *mm,
                dd: *dd,
                elapsed_ms: t0.elapsed().as_millis(),
                payload: DayPayload::Done {
                    rust_winner,
                    report,
                    perl_sections,
                },
            }
        })
        .collect();

    // Restore date order — par_iter result order is preserved with
    // `.collect()` but the cache files / progress log were written
    // in completion order. Sort by (mm, dd) so the manifest's
    // `days` array is stable across runs.
    day_results.sort_by_key(|r| (r.mm, r.dd));

    // ── Sequential aggregation pass. Walks per-day results and
    // merges into the global Stats + day_reports list.
    for r in day_results {
        stats.days_total += 1;
        match r.payload {
            DayPayload::Panic(msg) => {
                eprintln!("  PANIC {:02}-{:02}: {msg}", r.mm, r.dd);
                stats.panics.push((r.mm, r.dd, msg));
            }
            DayPayload::PerlFail(msg) => {
                eprintln!("  PERL-FAIL {:02}-{:02}: {msg}", r.mm, r.dd);
                stats.perl_failures.push((r.mm, r.dd, msg));
            }
            DayPayload::Done {
                rust_winner,
                report,
                perl_sections,
            } => {
                if report.winner_match {
                    stats.days_winner_match += 1;
                }
                for (idx, s) in report.sections.iter().enumerate() {
                    match s.status {
                        SectionStatus::Match => stats.section_match += 1,
                        SectionStatus::Differ => stats.section_differ += 1,
                        SectionStatus::RustBlank => stats.section_rust_blank += 1,
                        SectionStatus::PerlBlank => stats.section_perl_blank += 1,
                        SectionStatus::Empty => stats.section_empty += 1,
                    }
                    match s.category {
                        DivergenceCategory::MacroNotExpanded => stats.cat_macro_not_expanded += 1,
                        DivergenceCategory::RubricInjection => stats.cat_rubric_injection += 1,
                        DivergenceCategory::OrthoVariant => stats.cat_ortho_variant += 1,
                        DivergenceCategory::TrailingExtra => stats.cat_trailing_extra += 1,
                        DivergenceCategory::LeadingExtra => stats.cat_leading_extra += 1,
                        DivergenceCategory::Other => stats.cat_other += 1,
                        DivergenceCategory::Match
                        | DivergenceCategory::RustBlank
                        | DivergenceCategory::PerlBlank => {}
                    }
                    if idx < 12 {
                        stats.per_section_total[idx] += 1;
                        if matches!(s.status, SectionStatus::Match | SectionStatus::Empty) {
                            stats.per_section_pass[idx] += 1;
                        }
                    }
                    if matches!(s.status, SectionStatus::Differ | SectionStatus::RustBlank) {
                        if let Some(p_raw) = perl_sections.get(s.section) {
                            let p_clean = strip_perl_rubrics(&normalize(p_raw), s.section);
                            let hits: Vec<InferredSource> =
                                infer_perl_source(&p_clean, s.section);
                            if let Some(top) = hits.first() {
                                let key = format!("{}:{}", top.file, top.section);
                                *stats.inferred_misses.entry(key).or_insert(0) += 1;
                                let pair = format!("{} -> {}", rust_winner, top.file);
                                *stats.inferred_pairs.entry(pair).or_insert(0) += 1;
                            }
                        }
                    }
                }
                if report.is_pass() {
                    stats.days_passing += 1;
                }
                day_reports.push(report);
            }
        }
        let _ = r.elapsed_ms; // currently unused in stats; kept for future per-day metrics
    }

    // ── Reports.
    let manifest_path = out_dir.join("manifest.json");
    let board_path = out_dir.join("board.html");

    let total_section_cells = stats.section_match
        + stats.section_differ
        + stats.section_rust_blank
        + stats.section_perl_blank
        + stats.section_empty;
    let pass_pct = if stats.days_total == 0 {
        0.0
    } else {
        stats.days_passing as f32 / stats.days_total as f32 * 100.0
    };
    let section_match_pct = if total_section_cells == 0 {
        0.0
    } else {
        stats.section_match as f32 / total_section_cells as f32 * 100.0
    };

    let per_section: Vec<serde_json::Value> = PROPER_SECTIONS
        .iter()
        .enumerate()
        .map(|(idx, name)| {
            let pass = stats.per_section_pass[idx];
            let total = stats.per_section_total[idx];
            let pct = if total == 0 {
                0.0
            } else {
                pass as f32 / total as f32 * 100.0
            };
            serde_json::json!({
                "section": name,
                "pass": pass,
                "total": total,
                "pct": pct,
            })
        })
        .collect();
    // Top-N inferred-source aggregations.
    let top_misses = top_n(&stats.inferred_misses, 20);
    let top_pairs = top_n(&stats.inferred_pairs, 20);

    let manifest = serde_json::json!({
        "rubric": cfg.rubric_str,
        "year": year,
        "phase": "6",
        "tool": "year_sweep",
        "tool_version": env!("CARGO_PKG_VERSION"),
        "elapsed_seconds": started.elapsed().as_secs_f64(),
        "stats": {
            "days_total":          stats.days_total,
            "days_passing":        stats.days_passing,
            "days_winner_match":   stats.days_winner_match,
            "section_match":       stats.section_match,
            "section_differ":      stats.section_differ,
            "section_rust_blank":  stats.section_rust_blank,
            "section_perl_blank":  stats.section_perl_blank,
            "section_empty":       stats.section_empty,
            "section_total":       total_section_cells,
            "cat_macro_not_expanded": stats.cat_macro_not_expanded,
            "cat_rubric_injection":   stats.cat_rubric_injection,
            "cat_ortho_variant":      stats.cat_ortho_variant,
            "cat_trailing_extra":     stats.cat_trailing_extra,
            "cat_leading_extra":      stats.cat_leading_extra,
            "cat_other":              stats.cat_other,
            "per_section":         per_section,
            "panics":              stats.panics.len(),
            "perl_failures":       stats.perl_failures.len(),
            "pass_pct":            pass_pct,
            "section_match_pct":   section_match_pct,
        },
        "inferred_top_misses": top_misses
            .iter()
            .map(|(k, v)| serde_json::json!({ "file_section": k, "count": v }))
            .collect::<Vec<_>>(),
        "inferred_top_pairs": top_pairs
            .iter()
            .map(|(k, v)| serde_json::json!({ "rust_to_perl": k, "count": v }))
            .collect::<Vec<_>>(),
        "panics": stats.panics.iter().map(|(m, d, e)| {
            serde_json::json!({ "date": format!("{:02}-{:02}", m, d), "error": e })
        }).collect::<Vec<_>>(),
        "perl_failures": stats.perl_failures.iter().map(|(m, d, e)| {
            serde_json::json!({ "date": format!("{:02}-{:02}", m, d), "error": e })
        }).collect::<Vec<_>>(),
        "days": day_reports,
    });
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest).unwrap())
        .expect("write manifest");

    fs::write(&board_path, render_board_html(&cfg, year, &day_reports, &stats, pass_pct, section_match_pct))
        .expect("write board");

    println!();
    println!("─── Phase 6 year-sweep summary ───");
    println!("  rubric:            {}", cfg.rubric_str);
    println!("  year:              {}", year);
    println!("  days passing:      {}/{} ({:.1}%)",
        stats.days_passing, stats.days_total, pass_pct);
    println!("  winner-match days: {}/{}", stats.days_winner_match, stats.days_total);
    println!("  section match:     {}/{} ({:.1}%)",
        stats.section_match, total_section_cells, section_match_pct);
    println!("  section differ:    {}", stats.section_differ);
    println!("    └ macro-not-expanded: {}", stats.cat_macro_not_expanded);
    println!("    └ rubric-injection:   {}", stats.cat_rubric_injection);
    println!("    └ ortho-variant:      {}", stats.cat_ortho_variant);
    println!("    └ trailing-extra:     {}", stats.cat_trailing_extra);
    println!("    └ leading-extra:      {}", stats.cat_leading_extra);
    println!("    └ other:              {}", stats.cat_other);
    println!("  rust blank:        {}", stats.section_rust_blank);
    println!("  perl blank:        {}", stats.section_perl_blank);
    println!("  panics:            {}", stats.panics.len());
    println!("  perl failures:     {}", stats.perl_failures.len());
    println!();
    println!("  per-section match-rate:");
    for (idx, name) in PROPER_SECTIONS.iter().enumerate() {
        let pass = stats.per_section_pass[idx];
        let total = stats.per_section_total[idx];
        let pct = if total == 0 { 0.0 } else { pass as f32 / total as f32 * 100.0 };
        println!("    {:14}  {:>3}/{:>3}  ({:>5.1}%)", name, pass, total, pct);
    }
    if !top_misses.is_empty() {
        println!();
        println!("  top inferred Perl-source files (Rust missed):");
        for (k, v) in top_misses.iter().take(10) {
            println!("    {:>4}× {}", v, k);
        }
    }
    if !top_pairs.is_empty() {
        println!();
        println!("  top rust-winner ⇒ perl-source pairs:");
        for (k, v) in top_pairs.iter().take(10) {
            println!("    {:>4}× {}", v, k);
        }
    }
    println!();
    println!("manifest: {}", manifest_path.display());
    println!("board:    {}", board_path.display());

    if pass_pct < 99.0 {
        eprintln!("\nNOTE: pass rate {pass_pct:.1}% < 99% threshold — see board for red cells.");
    }
    let ok = stats.panics.is_empty()
        && stats.perl_failures.is_empty()
        && stats.section_differ == 0
        && stats.section_rust_blank == 0;
    (
        stats.days_passing as u32,
        stats.days_total as u32,
        ok,
    )
}

/// Sort `(key, count)` pairs descending by count, take top `n`.
fn top_n<K: Clone>(map: &BTreeMap<K, usize>, n: usize) -> Vec<(K, usize)> {
    let mut v: Vec<(K, usize)> = map.iter().map(|(k, v)| (k.clone(), *v)).collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    v.truncate(n);
    v
}

fn panic_message(e: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = e.downcast_ref::<&str>() {
        return s.to_string();
    }
    if let Some(s) = e.downcast_ref::<String>() {
        return s.clone();
    }
    "unknown panic".to_string()
}

// ─── Board HTML ──────────────────────────────────────────────────────

fn render_board_html(
    cfg: &Cfg,
    year: i32,
    days: &[DayReport],
    stats: &Stats,
    pass_pct: f32,
    section_match_pct: f32,
) -> String {
    let mut h = String::new();
    write!(&mut h,
"<!doctype html><meta charset=utf-8>
<title>year-sweep {} {}</title>
<style>
body{{font:13px system-ui,-apple-system,Helvetica,sans-serif;margin:1.5em;color:#222;}}
h1{{margin:0 0 0.25em;}}
.summary{{margin:0.5em 0 1em;color:#555;font-size:90%;line-height:1.5;}}
table{{border-collapse:collapse;font-size:11px;}}
th,td{{padding:2px 6px;text-align:center;border:1px solid #ddd;}}
th{{background:#fafafa;font-weight:500;}}
td.date{{text-align:left;font-family:ui-monospace,Consolas,monospace;color:#444;}}
td.winner{{text-align:left;font-size:10px;color:#555;max-width:18em;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;}}
td.match{{background:#c8e6c9;}}
td.differ{{background:#ffcdd2;}}
td.rust-blank{{background:#fff9c4;}}
td.perl-blank{{background:#bbdefb;}}
td.empty{{background:#f5f5f5;color:#aaa;}}
td.win-mismatch{{background:#ff8a80;color:#a00;}}
tr:hover td:not(.empty){{outline:1px solid #888;}}
.legend span{{display:inline-block;padding:1px 6px;margin:0 4px 0 0;border:1px solid #ccc;font-size:11px;}}
</style>
<h1>year-sweep — {} — {}</h1>
<p class=summary>
Days passing: <b>{}/{} ({:.1}%)</b> &middot;
Winner-match days: {}/{} &middot;
Section match: <b>{}/{} ({:.1}%)</b>.
Differ: {} &middot; Rust blank: {} &middot; Perl blank: {} &middot;
Panics: {} &middot; Perl failures: {}.
</p>
<p class=legend>
<span class=match style='background:#c8e6c9'>match</span>
<span class=differ style='background:#ffcdd2'>differ</span>
<span class=rust-blank style='background:#fff9c4'>rust blank</span>
<span class=perl-blank style='background:#bbdefb'>perl blank</span>
<span class=empty style='background:#f5f5f5'>both empty</span>
<span class=win-mismatch style='background:#ff8a80'>winner mismatch</span>
</p>
<table>
<thead><tr><th>date</th><th>winner</th>",
        cfg.rubric_str, year,
        cfg.rubric_str, year,
        stats.days_passing, stats.days_total, pass_pct,
        stats.days_winner_match, stats.days_total,
        stats.section_match, stats.section_match + stats.section_differ + stats.section_rust_blank + stats.section_perl_blank + stats.section_empty,
        section_match_pct,
        stats.section_differ, stats.section_rust_blank, stats.section_perl_blank,
        stats.panics.len(), stats.perl_failures.len(),
    ).unwrap();
    for s in PROPER_SECTIONS {
        write!(&mut h, "<th title='{s}'>{}</th>", &s.chars().take(4).collect::<String>()).unwrap();
    }
    h.push_str("</tr></thead><tbody>");
    for d in days {
        let win_class = if d.winner_match { "winner" } else { "winner win-mismatch" };
        write!(&mut h,
            "<tr><td class=date>{}</td><td class='{}'>{}</td>",
            d.date,
            win_class,
            html_escape(&d.winner_rust),
        ).unwrap();
        for s in &d.sections {
            let cls = match s.status {
                SectionStatus::Match     => "match",
                SectionStatus::Differ    => "differ",
                SectionStatus::RustBlank => "rust-blank",
                SectionStatus::PerlBlank => "perl-blank",
                SectionStatus::Empty     => "empty",
            };
            let title = format!("rust={}b perl={}b", s.rust_len, s.perl_len);
            write!(&mut h, "<td class={cls} title='{title}'></td>").unwrap();
        }
        h.push_str("</tr>");
    }
    h.push_str("</tbody></table>");
    h
}

// ─── Per-day diff dump ───────────────────────────────────────────────

fn render_day_diff(
    date: &str,
    rust_winner: &str,
    rust_propers: &MassPropers,
    perl_html: &str,
    report: &DayReport,
) -> String {
    let perl_sections = extract_perl_sections(perl_html);
    let mut out = String::new();
    use std::fmt::Write as _;
    writeln!(&mut out, "# {date}\n").unwrap();
    writeln!(&mut out, "- Rust winner: `{}`", rust_winner).unwrap();
    writeln!(&mut out, "- Perl headline: `{}`", report.winner_perl).unwrap();
    writeln!(&mut out, "- Winner match: **{}**\n", report.winner_match).unwrap();

    for sect in PROPER_SECTIONS {
        let r_block = rust_block(rust_propers, sect);
        let r_raw = r_block.map(|b| b.latin.as_str()).unwrap_or("");
        let p_raw = perl_sections.get(*sect).map(String::as_str).unwrap_or("");
        let r_norm = normalize(r_raw);
        let p_norm = normalize(p_raw);
        let p_clean = strip_perl_rubrics(&p_norm, sect);
        let status = report
            .sections
            .iter()
            .find(|s| s.section == *sect)
            .map(|s| s.status)
            .unwrap_or(SectionStatus::Empty);

        writeln!(&mut out, "## {sect} — {:?}", status).unwrap();
        writeln!(
            &mut out,
            "    rust raw   ({:>5}b): {}",
            r_raw.len(),
            excerpt(r_raw, 200)
        )
        .unwrap();
        writeln!(
            &mut out,
            "    rust norm  ({:>5}c): {}",
            r_norm.chars().count(),
            excerpt(&r_norm, 200)
        )
        .unwrap();
        writeln!(
            &mut out,
            "    perl raw   ({:>5}b): {}",
            p_raw.len(),
            excerpt(p_raw, 200)
        )
        .unwrap();
        writeln!(
            &mut out,
            "    perl norm  ({:>5}c): {}",
            p_norm.chars().count(),
            excerpt(&p_norm, 200)
        )
        .unwrap();
        // The clean form is what the comparator actually consults —
        // surfaces it so a dump reader can compute the diff manually.
        if p_clean != p_norm {
            writeln!(
                &mut out,
                "    perl clean ({:>5}c): {}   (rubrics stripped)",
                p_clean.chars().count(),
                excerpt(&p_clean, 200)
            )
            .unwrap();
        }
        if matches!(status, SectionStatus::Differ | SectionStatus::RustBlank) {
            let div = explain_divergence(&r_norm, &p_clean);
            writeln!(
                &mut out,
                "    diverge at rust char {} (matched {} char prefix vs cleaned perl)",
                div.matched_prefix_len, div.matched_prefix_len
            )
            .unwrap();
            writeln!(&mut out, "      rust ctx: {}", excerpt(&div.rust_context, 80)).unwrap();
            writeln!(&mut out, "      perl ctx: {}", excerpt(&div.perl_context, 80)).unwrap();
            // Word-level hint: split into normalised "words" of length
            // 4-12 chars by sliding window. Helps when the divergence
            // is a single substituted word (genetrice vs genitrice).
            if let Some((rword, pword)) = first_diff_word(&r_norm, &p_clean) {
                writeln!(&mut out, "      single-word diff: rust={rword:?} perl={pword:?}").unwrap();
            }
            // Reverse-lookup: which corpus file is Perl effectively
            // using for this section? Surfaces "wrong-winner" /
            // "wrong-Tempora-variant" gaps cleanly. Cap at 3 hits to
            // avoid drowning the dump on prayers that recur (e.g.
            // common &Gloria-tail antiphons).
            let hits = infer_perl_source(&p_clean, sect);
            if !hits.is_empty() {
                writeln!(&mut out, "      perl-source candidates:").unwrap();
                for h in &hits {
                    writeln!(
                        &mut out,
                        "        {:>6.1}%  {}:{}",
                        h.score * 100.0,
                        h.file,
                        h.section,
                    )
                    .unwrap();
                }
            }
        }
        writeln!(&mut out).unwrap();
    }
    out
}

/// Heuristic single-word divergence locator. Slides through the
/// normalised strings looking for the smallest substituted run.
/// Returns the diverging chars on each side (up to 24 chars), with
/// some shared context trimmed so the user can eyeball the diff.
/// Returns None when the two strings are equal or one is a strict
/// prefix of the other.
fn first_diff_word(rust: &str, perl: &str) -> Option<(String, String)> {
    if rust == perl {
        return None;
    }
    let rb: &[u8] = rust.as_bytes();
    let pb: &[u8] = perl.as_bytes();
    let common = rb.iter().zip(pb.iter()).take_while(|(a, b)| a == b).count();
    // Walk forward until we re-sync (or hit end on either side).
    // Try resyncs of length 1..16; the smallest one wins.
    for resync_len in 4..=16 {
        for k in (common + 1)..rust.chars().count().min(common + 64) {
            // Position k bytes into rust; find a window of resync_len
            // chars that occurs in perl after `common`.
            let r_chars: Vec<char> = rust.chars().collect();
            if k + resync_len > r_chars.len() {
                break;
            }
            let needle: String = r_chars[k..k + resync_len].iter().collect();
            let p_chars: Vec<char> = perl.chars().collect();
            if let Some(p_pos) = subslice_find(&p_chars, &needle.chars().collect::<Vec<_>>(), common) {
                let r_word: String = r_chars[common..k].iter().collect();
                let p_word: String = p_chars[common..p_pos].iter().collect();
                if r_word != p_word {
                    return Some((excerpt(&r_word, 32), excerpt(&p_word, 32)));
                }
            }
        }
    }
    // Could not re-sync — fall back to "rest of both" up to 24 chars.
    let r_rest: String = rust.chars().skip(common).take(24).collect();
    let p_rest: String = perl.chars().skip(common).take(24).collect();
    if r_rest != p_rest {
        Some((r_rest, p_rest))
    } else {
        None
    }
}

fn subslice_find(haystack: &[char], needle: &[char], from: usize) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    let max = haystack.len() - needle.len();
    for i in from..=max {
        if &haystack[i..i + needle.len()] == needle {
            return Some(i);
        }
    }
    None
}

fn rust_block<'a>(p: &'a MassPropers, name: &str) -> Option<&'a officium_rs::core::ProperBlock> {
    match name {
        "Introitus" => p.introitus.as_ref(),
        "Oratio" => p.oratio.as_ref(),
        "Lectio" => p.lectio.as_ref(),
        "Graduale" => p.graduale.as_ref(),
        "Tractus" => p.tractus.as_ref(),
        "Sequentia" => p.sequentia.as_ref(),
        "Evangelium" => p.evangelium.as_ref(),
        "Offertorium" => p.offertorium.as_ref(),
        "Secreta" => p.secreta.as_ref(),
        "Prefatio" => p.prefatio.as_ref(),
        "Communio" => p.communio.as_ref(),
        "Postcommunio" => p.postcommunio.as_ref(),
        _ => None,
    }
}

fn excerpt(s: &str, max_chars: usize) -> String {
    let mut out = String::new();
    let mut taken = 0;
    for c in s.chars() {
        if taken >= max_chars {
            out.push('…');
            break;
        }
        match c {
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
        taken += 1;
    }
    out
}

fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}
