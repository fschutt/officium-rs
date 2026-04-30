//! getweek-check — Phase 2 Rust↔Perl diff for the temporal-cycle
//! `getweek()` function.
//!
//! Calls `scripts/perl_getweek_year.pl` once per run (one Perl process
//! per year — fast: a year of getweek calls is ~0.1 s of Perl), parses
//! its TSV, runs the Rust `getweek` for each `(month, day)`, and
//! reports any divergences.
//!
//! Usage:
//!   cargo run --bin getweek-check -- --year 2026
//!   cargo run --bin getweek-check -- --year 2026 --missa 0 --tomorrow 1
//!   cargo run --bin getweek-check -- --years 2024 2030    (range, inclusive)
//!
//! Exit codes:
//!   0   no divergences
//!   1   divergences found (printed to stdout)
//!   2   args error
//!   3   Perl harness failure
//!
//! See DIVINUM_OFFICIUM_PORT_PLAN.md Phase 2.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

use md2json2::divinum_officium::date;

fn repo_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p
}

fn usage() -> ! {
    eprintln!(
        "Usage:\n  \
         getweek-check [--year YYYY] [--years FROM TO] [--missa 0|1] [--tomorrow 0|1]\n\
         \n\
         Defaults: --year 2026 --missa 1 --tomorrow 0"
    );
    std::process::exit(2);
}

struct Cfg {
    years: Vec<i32>,
    missa: u8,
    tomorrow: u8,
}

fn parse_args() -> Cfg {
    let mut years: Vec<i32> = vec![];
    let mut missa: u8 = 1;
    let mut tomorrow: u8 = 0;
    let mut single_year: Option<i32> = None;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--year" => {
                i += 1;
                single_year = Some(
                    args.get(i)
                        .and_then(|s| s.parse().ok())
                        .unwrap_or_else(|| usage()),
                );
            }
            "--years" => {
                i += 1;
                let from: i32 = args.get(i).and_then(|s| s.parse().ok()).unwrap_or_else(|| usage());
                i += 1;
                let to: i32 = args.get(i).and_then(|s| s.parse().ok()).unwrap_or_else(|| usage());
                if to < from {
                    usage();
                }
                years.extend(from..=to);
            }
            "--missa" => {
                i += 1;
                missa = args
                    .get(i)
                    .and_then(|s| s.parse().ok())
                    .filter(|v| *v <= 1u8)
                    .unwrap_or_else(|| usage());
            }
            "--tomorrow" => {
                i += 1;
                tomorrow = args
                    .get(i)
                    .and_then(|s| s.parse().ok())
                    .filter(|v| *v <= 1u8)
                    .unwrap_or_else(|| usage());
            }
            "-h" | "--help" => usage(),
            other => {
                eprintln!("unknown arg: {other}");
                usage();
            }
        }
        i += 1;
    }

    if years.is_empty() {
        years.push(single_year.unwrap_or(2026));
    }
    Cfg {
        years,
        missa,
        tomorrow,
    }
}

fn fetch_perl_labels(year: i32, missa: u8, tomorrow: u8) -> BTreeMap<(u32, u32), (u32, String)> {
    let script = repo_root().join("scripts/perl_getweek_year.pl");
    if !script.exists() {
        eprintln!("FATAL: {} missing", script.display());
        std::process::exit(3);
    }
    let out = Command::new("perl")
        .arg(&script)
        .arg(year.to_string())
        .arg(missa.to_string())
        .arg(tomorrow.to_string())
        .output()
        .expect("spawn perl");
    if !out.status.success() {
        eprintln!(
            "FATAL: perl harness exit {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        );
        std::process::exit(3);
    }

    let mut map = BTreeMap::new();
    for line in std::str::from_utf8(&out.stdout).expect("perl emitted non-utf8").lines() {
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() != 3 {
            continue;
        }
        let mmdd: Vec<&str> = cols[0].split('-').collect();
        if mmdd.len() != 2 {
            continue;
        }
        let m: u32 = mmdd[0].parse().expect("perl bad month");
        let d: u32 = mmdd[1].parse().expect("perl bad day");
        let dow: u32 = cols[1].parse().expect("perl bad dow");
        map.insert((m, d), (dow, cols[2].to_string()));
    }
    map
}

fn check_year(year: i32, missa: u8, tomorrow: u8) -> usize {
    let perl = fetch_perl_labels(year, missa, tomorrow);
    let mut bad: Vec<(u32, u32, String, String)> = Vec::new();
    let mut dow_bad: Vec<(u32, u32, u32, u32)> = Vec::new();
    let mut count = 0;
    for ((m, d), (perl_dow, perl_label)) in &perl {
        count += 1;
        // Cross-check day_of_week first — if the underlying date math
        // disagrees, getweek divergences are noise downstream.
        let rust_dow = date::day_of_week(*d, *m, year);
        if rust_dow != *perl_dow {
            dow_bad.push((*m, *d, rust_dow, *perl_dow));
        }
        let rust_label = date::getweek(*d, *m, year, tomorrow != 0, missa != 0);
        if rust_label != *perl_label {
            bad.push((*m, *d, rust_label, perl_label.clone()));
        }
    }
    println!(
        "year={year} missa={missa} tomorrow={tomorrow}: {count} dates, \
         {} dow mismatches, {} getweek mismatches",
        dow_bad.len(),
        bad.len()
    );
    for (m, d, r, p) in &dow_bad {
        println!("  DOW {year}-{m:02}-{d:02}: rust={r} perl={p}");
    }
    for (m, d, r, p) in &bad {
        println!("  WK  {year}-{m:02}-{d:02}: rust={r:?} perl={p:?}");
    }
    bad.len() + dow_bad.len()
}

fn main() {
    let cfg = parse_args();
    let mut total_bad = 0usize;
    for y in &cfg.years {
        total_bad += check_year(*y, cfg.missa, cfg.tomorrow);
    }
    if total_bad == 0 {
        println!("OK — no divergences across {} year(s)", cfg.years.len());
    } else {
        println!("FAIL — {} divergences total", total_bad);
        std::process::exit(1);
    }
}
