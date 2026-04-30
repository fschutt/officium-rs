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

use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Instant;

use md2json2::divinum_officium::core::{Date, Locale, MassPropers, OfficeInput, Rubric};
use md2json2::divinum_officium::corpus::BundledCorpus;
use md2json2::divinum_officium::mass::mass_propers;
use md2json2::divinum_officium::precedence::compute_office;
use md2json2::divinum_officium::regression::{
    compare_day, explain_divergence, extract_perl_sections, normalize, DayReport,
    DivergenceCategory, SectionStatus, PROPER_SECTIONS,
};

const KNOWN_RUBRICS: &[(&str, Rubric)] = &[
    ("Tridentine - 1570",   Rubric::Tridentine1570),
    ("Tridentine - 1910",   Rubric::Tridentine1910),
    ("Divino Afflatu",      Rubric::DivinoAfflatu1911),
    ("Reduced - 1955",      Rubric::Reduced1955),
    ("Rubrics 1960 - 1960", Rubric::Rubrics1960),
    ("pre-Trident Monastic", Rubric::Monastic),
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
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p
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
    cat_other: usize,
    /// Per-section pass rates (Match + Empty / total).
    per_section_pass: [usize; 12],
    per_section_total: [usize; 12],
    panics: Vec<(u32, u32, String)>,
    perl_failures: Vec<(u32, u32, String)>,
}

struct Cfg {
    year: i32,
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
}

fn parse_args() -> Cfg {
    let mut year: Option<i32> = None;
    let mut rubric_str = String::from("Tridentine - 1570");
    let mut limit: Option<usize> = None;
    let mut smoke = false;
    let mut out_override: Option<PathBuf> = None;
    let mut dump = false;
    let mut progress = true;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--year" => {
                i += 1;
                year = Some(args.get(i).and_then(|s| s.parse().ok()).unwrap_or_else(|| usage()));
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
        rubric_str,
        rubric,
        limit,
        smoke,
        out_override,
        dump,
        progress,
    }
}

fn main() {
    let cfg = parse_args();
    let root = repo_root();
    assert_vendor_present(&root);

    let dates: Vec<(u32, u32)> = if cfg.smoke {
        vec![(1, 1), (4, 30), (12, 25)]
    } else {
        let mut d = dates_for_year(cfg.year);
        if let Some(n) = cfg.limit {
            d.truncate(n);
        }
        d
    };

    let slug = slugify_rubric(&cfg.rubric_str);
    let out_dir = cfg
        .out_override
        .clone()
        .unwrap_or_else(|| root.join(format!("md2json2/target/regression/{slug}-{}", cfg.year)));
    fs::create_dir_all(&out_dir).expect("create output dir");

    let total = dates.len();
    let started = Instant::now();
    let mut stats = Stats::default();
    let mut day_reports: Vec<DayReport> = Vec::with_capacity(total);

    println!(
        "year-sweep Phase 6: rubric={:?} year={} dates={} out={}",
        cfg.rubric_str,
        cfg.year,
        total,
        out_dir.display()
    );

    for (i, (mm, dd)) in dates.iter().enumerate() {
        let t0 = Instant::now();
        let date_label = format!("{:04}-{:02}-{:02}", cfg.year, mm, dd);
        stats.days_total += 1;

        // ── 1. Rust pipeline (catch panics — Phase 3-5 ports
        // intentionally call panic!() on unsupported rubrics or
        // unknown corpus shapes; we don't want to abort the sweep).
        let input = OfficeInput {
            date: Date::new(cfg.year, *mm, *dd),
            rubric: cfg.rubric,
            locale: Locale::Latin,
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
                stats.panics.push((*mm, *dd, msg.clone()));
                eprintln!("  PANIC {:02}-{:02}: {msg}", mm, dd);
                continue;
            }
        };

        // ── 2. Perl pipeline.
        let perl_html = match render_perl(&root, *mm, *dd, cfg.year, &cfg.rubric_str) {
            Ok(h) => h,
            Err(e) => {
                stats.perl_failures.push((*mm, *dd, e.clone()));
                eprintln!("  PERL-FAIL {:02}-{:02}: {e}", mm, dd);
                continue;
            }
        };
        // Cache the raw HTML for human inspection later.
        let html_path = out_dir.join(format!("{:02}-{:02}.perl.html", mm, dd));
        let _ = fs::write(&html_path, &perl_html);

        // ── 3. Compare.
        let report = compare_day(&date_label, &rust_winner, &rust_propers, &perl_html);
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
        }
        if report.is_pass() {
            stats.days_passing += 1;
        }

        // ── 3b. Per-day diff dump (--dump).
        if cfg.dump {
            let dump_path = out_dir.join(format!("{:02}-{:02}.diff.md", mm, dd));
            let dump_text =
                render_day_diff(&date_label, &rust_winner, &rust_propers, &perl_html, &report);
            let _ = fs::write(&dump_path, dump_text);
        }

        day_reports.push(report);

        if cfg.progress && (i % 25 == 0 || i + 1 == total) {
            let pass_now = stats.section_match;
            let total_now = stats.section_match
                + stats.section_differ
                + stats.section_rust_blank
                + stats.section_perl_blank;
            let live_pct = if total_now > 0 {
                pass_now as f32 / total_now as f32 * 100.0
            } else {
                0.0
            };
            println!(
                "  [{:>3}/{:>3}] {:5.1}% {:02}-{:02}  match={:.1}%  ({} ms)",
                i + 1,
                total,
                (i + 1) as f32 / total as f32 * 100.0,
                mm,
                dd,
                live_pct,
                t0.elapsed().as_millis(),
            );
        }
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
    let manifest = serde_json::json!({
        "rubric": cfg.rubric_str,
        "year": cfg.year,
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
            "cat_other":              stats.cat_other,
            "per_section":         per_section,
            "panics":              stats.panics.len(),
            "perl_failures":       stats.perl_failures.len(),
            "pass_pct":            pass_pct,
            "section_match_pct":   section_match_pct,
        },
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

    fs::write(&board_path, render_board_html(&cfg, &day_reports, &stats, pass_pct, section_match_pct))
        .expect("write board");

    println!();
    println!("─── Phase 6 year-sweep summary ───");
    println!("  rubric:            {}", cfg.rubric_str);
    println!("  year:              {}", cfg.year);
    println!("  days passing:      {}/{} ({:.1}%)",
        stats.days_passing, stats.days_total, pass_pct);
    println!("  winner-match days: {}/{}", stats.days_winner_match, stats.days_total);
    println!("  section match:     {}/{} ({:.1}%)",
        stats.section_match, total_section_cells, section_match_pct);
    println!("  section differ:    {}", stats.section_differ);
    println!("    └ macro-not-expanded: {}", stats.cat_macro_not_expanded);
    println!("    └ rubric-injection:   {}", stats.cat_rubric_injection);
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
    println!();
    println!("manifest: {}", manifest_path.display());
    println!("board:    {}", board_path.display());

    if pass_pct < 99.0 {
        eprintln!("\nNOTE: pass rate {pass_pct:.1}% < 99% threshold — see board for red cells.");
    }
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
        cfg.rubric_str, cfg.year,
        cfg.rubric_str, cfg.year,
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
        let status = report
            .sections
            .iter()
            .find(|s| s.section == *sect)
            .map(|s| s.status)
            .unwrap_or(SectionStatus::Empty);

        writeln!(&mut out, "## {sect} — {:?}", status).unwrap();
        writeln!(
            &mut out,
            "    rust raw  ({:>5}b): {}",
            r_raw.len(),
            excerpt(r_raw, 200)
        )
        .unwrap();
        writeln!(
            &mut out,
            "    rust norm ({:>5}c): {}",
            r_norm.chars().count(),
            excerpt(&r_norm, 200)
        )
        .unwrap();
        writeln!(
            &mut out,
            "    perl raw  ({:>5}b): {}",
            p_raw.len(),
            excerpt(p_raw, 200)
        )
        .unwrap();
        writeln!(
            &mut out,
            "    perl norm ({:>5}c): {}",
            p_norm.chars().count(),
            excerpt(&p_norm, 200)
        )
        .unwrap();
        if matches!(status, SectionStatus::Differ) {
            let div = explain_divergence(&r_norm, &p_norm);
            writeln!(
                &mut out,
                "    diverge at rust char {} (matched {} char prefix)",
                div.matched_prefix_len, div.matched_prefix_len
            )
            .unwrap();
            writeln!(&mut out, "      rust ctx: {}", excerpt(&div.rust_context, 80)).unwrap();
            writeln!(&mut out, "      perl ctx: {}", excerpt(&div.perl_context, 80)).unwrap();
        }
        writeln!(&mut out).unwrap();
    }
    out
}

fn rust_block<'a>(p: &'a MassPropers, name: &str) -> Option<&'a md2json2::divinum_officium::core::ProperBlock> {
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
