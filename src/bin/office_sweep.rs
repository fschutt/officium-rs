//! office_sweep — Breviary regression-loop driver (B8).
//!
//! Single-cell sweep. For one (date, rubric, hour, day_key) tuple:
//!
//!   1. **Rust pipeline**: build `OfficeArgs` → `compute_office_hour`.
//!      Captures the structured `Vec<RenderedLine>` for the hour.
//!   2. **Perl pipeline**: shell to `scripts/do_render.sh DATE
//!      VERSION HOUR`. Captures the upstream HTML.
//!   3. **Comparison**: extract the named section's body from each
//!      side via `regression::rust_office_section` /
//!      `regression::extract_perl_sections`, then run
//!      `compare_section_named` for a verdict.
//!
//! This is slice 1 of B8 — proves the Perl/Rust round-trip and
//! prints a single-cell verdict. Multi-day, multi-hour, multi-rubric
//! looping lands in slice 2.
//!
//! Usage:
//!
//!   cargo run --bin office-sweep -- --date 05-04-2026 --hour Vespera \
//!     --rubric 'Tridentine - 1570' --day-key Sancti/05-04 \
//!     --section Oratio
//!
//! Mass (year_sweep.rs) and Office (this) share `do_render.sh`; the
//! only difference is the HOUR argument.

use std::path::PathBuf;
use std::process::{Command, Stdio};

use officium_rs::core::Rubric;
use officium_rs::horas::{self, OfficeArgs};
use officium_rs::regression::{
    compare_office_section, rust_office_section, SectionStatus,
};

/// Mirror of year_sweep's rubric table — the Perl-name on the left,
/// the Rust enum on the right.
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

#[derive(Default)]
struct Args {
    date: String,         // MM-DD-YYYY (Perl-side US format)
    hour: String,         // Vespera, Matutinum, …
    rubric: String,       // "Tridentine - 1570" etc.
    day_key: String,      // Sancti/05-04
    next_day_key: String, // optional
    section: String,      // section name to compare (default: Oratio)
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        date: String::new(),
        hour: "Vespera".to_string(),
        rubric: "Tridentine - 1570".to_string(),
        day_key: String::new(),
        next_day_key: String::new(),
        section: "Oratio".to_string(),
    };
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < raw.len() {
        match raw[i].as_str() {
            "--date" => { i += 1; args.date = raw.get(i).cloned().unwrap_or_default(); }
            "--hour" => { i += 1; args.hour = raw.get(i).cloned().unwrap_or_default(); }
            "--rubric" => { i += 1; args.rubric = raw.get(i).cloned().unwrap_or_default(); }
            "--day-key" => { i += 1; args.day_key = raw.get(i).cloned().unwrap_or_default(); }
            "--next-day-key" => { i += 1; args.next_day_key = raw.get(i).cloned().unwrap_or_default(); }
            "--section" => { i += 1; args.section = raw.get(i).cloned().unwrap_or_default(); }
            "--help" | "-h" => {
                eprintln!(
                    "Usage: office_sweep --date MM-DD-YYYY --hour HOUR --rubric RUBRIC \\\n\
                     \t--day-key SANCTI/MM-DD --section SECTION\n\
                     \n\
                     Defaults: --hour Vespera, --rubric 'Tridentine - 1570', --section Oratio.\n\
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
    if args.date.is_empty() {
        return Err("--date is required".into());
    }
    if args.day_key.is_empty() {
        return Err("--day-key is required".into());
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

fn main() -> Result<(), String> {
    let args = parse_args()?;
    let rubric = parse_rubric(&args.rubric).ok_or_else(|| {
        format!(
            "unknown rubric {:?}; known: {}",
            args.rubric,
            KNOWN_RUBRICS.iter().map(|(s, _)| *s).collect::<Vec<_>>().join(", ")
        )
    })?;
    let (mm, dd, yyyy) = parse_us_date(&args.date)?;

    let repo_root = std::env::current_dir().map_err(|e| format!("cwd: {e}"))?;

    // Rust side.
    let day_key_owned = args.day_key.clone();
    let next_day_key_owned = args.next_day_key.clone();
    let resolved_key = if args.hour == "Vespera" && !next_day_key_owned.is_empty() {
        horas::first_vespers_day_key(&day_key_owned, &next_day_key_owned).to_string()
    } else {
        day_key_owned.clone()
    };
    let office_args = OfficeArgs {
        year: yyyy,
        month: mm,
        day: dd,
        rubric,
        hour: &args.hour,
        rubrics: true,
        day_key: Some(&resolved_key),
    };
    let lines = horas::compute_office_hour(&office_args);
    let rust_section = rust_office_section(&lines, &args.section);

    // Perl side.
    let perl_html = render_perl_office(&repo_root, &args.date, &args.rubric, &args.hour)?;

    // Compare.
    let rust_body = rust_section.clone().unwrap_or_default();
    let status = compare_office_section(&rust_body, &perl_html, &args.section);

    println!("date:     {}", args.date);
    println!("rubric:   {}", args.rubric);
    println!("hour:     {}", args.hour);
    println!("day_key:  {} (resolved → {})", args.day_key, resolved_key);
    println!("section:  {}", args.section);
    println!("rust_lines: {}", lines.len());
    println!("rust_body_present: {}", rust_section.is_some());
    println!(
        "rust_body_preview: {}",
        rust_body.chars().take(120).collect::<String>().replace('\n', " ⏎ ")
    );
    println!("verdict:  {status:?}");
    match status {
        SectionStatus::Match => Ok(()),
        SectionStatus::Empty => Err("section empty on both sides".into()),
        SectionStatus::Differ => Err("Rust and Perl disagree".into()),
        SectionStatus::RustBlank => Err("Rust emitted nothing for this section".into()),
        SectionStatus::PerlBlank => Err("Perl emitted nothing for this section".into()),
    }
}
