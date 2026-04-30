//! year_sweep — regression-loop driver for the Divinum Officium port.
//!
//! Phase 0 stub: renders a full year of Mass HTML via the upstream
//! Perl oracle (`scripts/do_render.sh`) and writes each day's output
//! plus a JSON manifest. Phase 6 will add the Rust-side
//! `compute_office()` + `mass_propers()` call alongside, and the
//! per-section diff that turns this into a real regression board.
//!
//! Usage:
//!   cargo run --bin year-sweep -- --year 2026 --rubric 'Tridentine - 1570'
//!   cargo run --bin year-sweep -- --smoke              (3 dates, sanity check)
//!   cargo run --bin year-sweep -- --year 2026 --limit 7
//!
//! Output:
//!   target/regression/{rubric-slug}-{year}/{MM-DD}.perl.html
//!   target/regression/{rubric-slug}-{year}/manifest.json
//!
//! See DIVINUM_OFFICIUM_PORT_PLAN.md Phase 0 / Phase 6.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Instant;

const KNOWN_RUBRICS: &[&str] = &[
    "Tridentine - 1570",
    "Tridentine - 1910",
    "Divino Afflatu",
    "Reduced - 1955",
    "Rubrics 1960 - 1960",
    "pre-Trident Monastic",
];

fn usage() -> ! {
    eprintln!(
        "Usage: year-sweep [--year YYYY] [--rubric NAME] [--limit N] [--smoke] [--out DIR]\n\
         \n\
         Defaults: --year 2026, --rubric 'Tridentine - 1570', no limit.\n\
         \n\
         Known rubrics:"
    );
    for r in KNOWN_RUBRICS {
        eprintln!("    {r:?}");
    }
    std::process::exit(2);
}

fn repo_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop(); // out of md2json2/ → repo root
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
    // Collapse runs of underscores → single underscore.
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

fn render(root: &PathBuf, mm: u32, dd: u32, yyyy: i32, rubric: &str) -> Result<String, String> {
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

fn parse_args() -> (i32, String, Option<usize>, bool, Option<PathBuf>) {
    let mut year: i32 = 2026;
    let mut rubric = String::from("Tridentine - 1570");
    let mut limit: Option<usize> = None;
    let mut smoke = false;
    let mut out_override: Option<PathBuf> = None;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--year" => {
                i += 1;
                year = args.get(i).and_then(|s| s.parse().ok()).unwrap_or_else(|| usage());
            }
            "--rubric" => {
                i += 1;
                rubric = args.get(i).cloned().unwrap_or_else(|| usage());
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
            "-h" | "--help" => usage(),
            other => {
                eprintln!("unknown arg: {other}");
                usage();
            }
        }
        i += 1;
    }
    (year, rubric, limit, smoke, out_override)
}

fn main() {
    let (year, rubric, limit, smoke, out_override) = parse_args();
    let root = repo_root();
    assert_vendor_present(&root);

    let dates: Vec<(u32, u32)> = if smoke {
        // Three calibration dates that exercise different temporal slots:
        // Christmas (Class I sanctoral), Easter Sunday-window placeholder
        // (we don't compute Easter here yet), and an ordinary post-
        // Pentecost Sunday-adjacent feria.
        vec![(1, 1), (4, 30), (12, 25)]
    } else {
        let mut d = dates_for_year(year);
        if let Some(n) = limit {
            d.truncate(n);
        }
        d
    };

    let slug = slugify_rubric(&rubric);
    let out_dir = out_override.unwrap_or_else(|| {
        root.join(format!("md2json2/target/regression/{slug}-{year}"))
    });
    fs::create_dir_all(&out_dir).expect("create output dir");

    let total = dates.len();
    let started = Instant::now();
    let mut succeeded = 0usize;
    let mut failed: Vec<(u32, u32, String)> = Vec::new();

    println!(
        "year-sweep: rubric={rubric:?} year={year} dates={total} out={}",
        out_dir.display()
    );

    for (i, (mm, dd)) in dates.iter().enumerate() {
        let t0 = Instant::now();
        match render(&root, *mm, *dd, year, &rubric) {
            Ok(html) => {
                let path = out_dir.join(format!("{:02}-{:02}.perl.html", mm, dd));
                fs::write(&path, &html).expect("write html");
                succeeded += 1;
                if i % 25 == 0 || i + 1 == total {
                    let pct = (i + 1) as f32 / total as f32 * 100.0;
                    println!(
                        "  [{:>3}/{:>3}] {:5.1}% {:02}-{:02} ({} ms, {} bytes)",
                        i + 1,
                        total,
                        pct,
                        mm,
                        dd,
                        t0.elapsed().as_millis(),
                        html.len(),
                    );
                }
            }
            Err(e) => {
                failed.push((*mm, *dd, e.clone()));
                eprintln!("  FAIL {:02}-{:02}: {e}", mm, dd);
            }
        }
    }

    let manifest_path = out_dir.join("manifest.json");
    let manifest = serde_json::json!({
        "rubric": rubric,
        "year": year,
        "phase": "0",
        "phase_note": "Phase 0 stub — Perl-only output cached. \
                       No Rust comparison until Phase 6.",
        "dates_attempted": total,
        "dates_succeeded": succeeded,
        "dates_failed": failed.len(),
        "failures": failed.iter().map(|(m, d, e)| {
            serde_json::json!({ "date": format!("{:02}-{:02}", m, d), "error": e })
        }).collect::<Vec<_>>(),
        "elapsed_seconds": started.elapsed().as_secs_f64(),
        "tool": "year_sweep",
        "tool_version": env!("CARGO_PKG_VERSION"),
    });
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest).unwrap())
        .expect("write manifest");

    println!(
        "\n{}/{} OK in {:.1}s — manifest: {}",
        succeeded,
        total,
        started.elapsed().as_secs_f64(),
        manifest_path.display()
    );
    if !failed.is_empty() {
        eprintln!("{} failures — see manifest", failed.len());
        std::process::exit(1);
    }
}
