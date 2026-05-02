//! year-kalendar — CLI for the data-driven year-aware kalendar.
//!
//! Lists the resolved Sancti table for any historical year by
//! consulting `kalendaria_layers::Layer::for_year(year)` and walking
//! every (month, day) in the active layer's diff-resolved table.
//!
//! Usage:
//!
//!   year-kalendar 1700                    # full year, 1570 baseline
//!   year-kalendar 1900                    # 1888 layer (Pius IX/Leo XIII era)
//!   year-kalendar 1956                    # 1955 layer (Pius XII Reduced)
//!   year-kalendar 2026                    # 1960 layer (John XXIII)
//!   year-kalendar 1700 03-25              # single date — Annunciation in 1700
//!   year-kalendar 1700 --diff             # show diff vs the previous layer
//!   year-kalendar --layers                # list every layer + its date range
//!
//! Demonstrates that adding a new reform calendar is a *data-only*
//! change: drop a new row into `data/kalendaria_by_rubric.json`,
//! extend the `Layer` enum, and this tool plus every other Sancti
//! consumer picks it up automatically.

use officium_rs::kalendaria_layers::{self, Cell, Layer};

fn usage() -> ! {
    eprintln!(
        "Usage: year-kalendar <year> [MM-DD]\n\
         \n\
         Examples:\n\
           year-kalendar 1700              # full year (Pius1570 layer)\n\
           year-kalendar 1956 05-31        # Mary Queen on May 31 in 1956\n\
           year-kalendar --layers          # list every layer + its year range\n\
           year-kalendar 1700 --diff       # show what 1888 layer changed\n\
           year-kalendar --canonization 'In Conversione S. Pauli'   # find a saint's history\n\
        "
    );
    std::process::exit(2);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        usage();
    }
    match args[0].as_str() {
        "--layers" => print_layers(),
        "--canonization" if args.len() >= 2 => {
            print_canonization_search(&args[1..].join(" "));
        }
        first => match first.parse::<i32>() {
            Ok(year) => {
                let layer = kalendaria_layers::layer_for_year(year);
                if args.len() == 1 {
                    print_year(year, layer);
                } else if args[1] == "--diff" {
                    print_layer_diff(layer);
                } else {
                    print_date(year, layer, &args[1]);
                }
            }
            Err(_) => usage(),
        },
    }
}

fn layer_label(layer: Layer) -> &'static str {
    match layer {
        Layer::Pius1570 => "Pius V (1570) baseline",
        Layer::LeoXIII1888 => "Leo XIII / Pius IX era (1888)",
        Layer::PiusX1906 => "Pius X early reforms (1906)",
        Layer::PiusXI1939 => "Pius XI updates (1939)",
        Layer::PiusXIIPre1954 => "Pius XII pre-Reduced (1954)",
        Layer::PiusXII1955 => "Pius XII Reduced (Cum nostra hac aetate, 1955)",
        Layer::JohnXXIII1960 => "John XXIII Rubrics (1960)",
    }
}

fn layer_year_range(layer: Layer) -> &'static str {
    match layer {
        Layer::Pius1570 => "1570 — 1887",
        Layer::LeoXIII1888 => "1888 — 1905",
        Layer::PiusX1906 => "1906 — 1938",
        Layer::PiusXI1939 => "1939 — 1953",
        Layer::PiusXIIPre1954 => "1954",
        Layer::PiusXII1955 => "1955 — 1959",
        Layer::JohnXXIII1960 => "1960 — present",
    }
}

fn print_layers() {
    println!("Reform layers (data: data/kalendaria_by_rubric.json):");
    println!();
    for layer in [
        Layer::Pius1570,
        Layer::LeoXIII1888,
        Layer::PiusX1906,
        Layer::PiusXI1939,
        Layer::PiusXIIPre1954,
        Layer::PiusXII1955,
        Layer::JohnXXIII1960,
    ] {
        println!(
            "  {:<24}  {:<14}  {}",
            layer.key(),
            layer_year_range(layer),
            layer_label(layer)
        );
    }
}

fn print_year(year: i32, layer: Layer) {
    println!(
        "Resolved Sancti calendar for {year} ({} — {})",
        layer.key(),
        layer_label(layer)
    );
    println!();
    let mut count = 0;
    for mm in 1..=12u32 {
        let max = if mm == 2 { 29 } else { 31 };
        for dd in 1..=max {
            if let Some(entry) = kalendaria_layers::lookup(layer, mm, dd) {
                if let Some(main) = entry.iter().find(|c| c.is_main()) {
                    print_cell_line(mm, dd, main, entry);
                    count += 1;
                }
            }
        }
    }
    println!();
    println!("  ({count} feasts in the kalendar; remaining days are ferial)");
}

fn print_cell_line(mm: u32, dd: u32, main: &Cell, entry: &[Cell]) {
    let comm_count = entry.iter().filter(|c| !c.is_main()).count();
    if comm_count == 0 {
        println!(
            "  {:02}-{:02}  {:<6}  {:<26} {}",
            mm,
            dd,
            main.rank_label,
            main.stem,
            main.officium
        );
    } else {
        println!(
            "  {:02}-{:02}  {:<6}  {:<26} {}",
            mm,
            dd,
            main.rank_label,
            main.stem,
            main.officium
        );
        for c in entry.iter().filter(|c| !c.is_main()) {
            println!(
                "          comm    {:<26} {}",
                c.stem,
                c.officium
            );
        }
    }
}

fn print_date(year: i32, layer: Layer, mm_dd: &str) {
    let parts: Vec<&str> = mm_dd.split('-').collect();
    if parts.len() != 2 {
        eprintln!("error: date must be MM-DD (got {mm_dd:?})");
        std::process::exit(2);
    }
    let mm: u32 = match parts[0].parse() {
        Ok(v) => v,
        Err(_) => {
            eprintln!("error: invalid month {:?}", parts[0]);
            std::process::exit(2);
        }
    };
    let dd: u32 = match parts[1].parse() {
        Ok(v) => v,
        Err(_) => {
            eprintln!("error: invalid day {:?}", parts[1]);
            std::process::exit(2);
        }
    };
    println!(
        "{:04}-{:02}-{:02} under layer {} ({}):",
        year,
        mm,
        dd,
        layer.key(),
        layer_label(layer)
    );
    println!();
    match kalendaria_layers::lookup(layer, mm, dd) {
        Some(entry) => {
            for cell in entry {
                let kind = if cell.is_main() { "main" } else { "comm" };
                println!(
                    "  [{kind}] stem={:<14} rank={:<3} {} {}",
                    cell.stem,
                    cell.rank,
                    cell.rank_label,
                    cell.officium,
                );
            }
        }
        None => {
            println!("  (no feast — ferial of the temporal cycle)");
        }
    }
}

fn print_layer_diff(layer: Layer) {
    let prior = match layer {
        Layer::Pius1570 => {
            println!("Pius1570 is the baseline — no diff target.");
            return;
        }
        Layer::LeoXIII1888 => Layer::Pius1570,
        Layer::PiusX1906 => Layer::LeoXIII1888,
        Layer::PiusXI1939 => Layer::PiusX1906,
        Layer::PiusXIIPre1954 => Layer::PiusXI1939,
        Layer::PiusXII1955 => Layer::PiusXIIPre1954,
        Layer::JohnXXIII1960 => Layer::PiusXII1955,
    };
    println!(
        "Differences {:?} → {:?}",
        prior, layer
    );
    println!();
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut changed = Vec::new();
    for mm in 1..=12u32 {
        let max = if mm == 2 { 29 } else { 31 };
        for dd in 1..=max {
            let p = kalendaria_layers::lookup(prior, mm, dd);
            let n = kalendaria_layers::lookup(layer, mm, dd);
            match (p, n) {
                (None, Some(e)) => added.push((mm, dd, e)),
                (Some(e), None) => removed.push((mm, dd, e)),
                (Some(a), Some(b)) if cell_cmp_key(a) != cell_cmp_key(b) => {
                    changed.push((mm, dd, a, b));
                }
                _ => {}
            }
        }
    }
    println!("Added ({}):", added.len());
    for (mm, dd, e) in &added {
        if let Some(main) = e.iter().find(|c| c.is_main()) {
            println!("  + {:02}-{:02}  {:<6}  {}", mm, dd, main.rank_label, main.officium);
        }
    }
    println!();
    println!("Removed ({}):", removed.len());
    for (mm, dd, e) in &removed {
        if let Some(main) = e.iter().find(|c| c.is_main()) {
            println!("  - {:02}-{:02}  {:<6}  {}", mm, dd, main.rank_label, main.officium);
        }
    }
    println!();
    println!("Changed ({}):", changed.len());
    for (mm, dd, a, b) in &changed {
        let am = a.iter().find(|c| c.is_main()).map(|c| c.officium.as_str()).unwrap_or("?");
        let bm = b.iter().find(|c| c.is_main()).map(|c| c.officium.as_str()).unwrap_or("?");
        let ar = a.iter().find(|c| c.is_main()).map(|c| c.rank.as_str()).unwrap_or("?");
        let br = b.iter().find(|c| c.is_main()).map(|c| c.rank.as_str()).unwrap_or("?");
        if am == bm {
            println!("  ~ {:02}-{:02}  rank {ar}→{br}  {}", mm, dd, am);
        } else {
            println!("  ~ {:02}-{:02}  {} (rank {ar}) → {} (rank {br})", mm, dd, am, bm);
        }
    }
}

fn cell_cmp_key(cells: &[Cell]) -> Vec<(String, String, String, String)> {
    cells
        .iter()
        .map(|c| (c.stem.clone(), c.officium.clone(), c.rank.clone(), c.kind.clone()))
        .collect()
}

// ─── Canonization search ───────────────────────────────────────────

fn print_canonization_search(needle: &str) {
    static CANONIZATION: &str = include_str!("../../data/canonization_dates.json");
    let parsed: serde_json::Value = match serde_json::from_str(CANONIZATION) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error parsing canonization data: {e}");
            std::process::exit(1);
        }
    };
    let map = match parsed.as_object() {
        Some(m) => m,
        None => {
            eprintln!("error: canonization data isn't a JSON object");
            std::process::exit(1);
        }
    };
    let needle_lower = needle.to_ascii_lowercase();
    let mut hits: Vec<(&String, &serde_json::Value)> = map
        .iter()
        .filter(|(_k, v)| {
            v.get("first_officium")
                .and_then(|s| s.as_str())
                .map(|s| s.to_ascii_lowercase().contains(&needle_lower))
                .unwrap_or(false)
        })
        .collect();
    hits.sort_by_key(|(k, _)| k.as_str());
    println!("Saints matching {needle:?} ({} hits):", hits.len());
    println!();
    for (key, v) in hits {
        let added = v.get("added_in_rubric").and_then(|s| s.as_str()).unwrap_or("?");
        let suppressed = v.get("suppressed_in_rubric").and_then(|s| s.as_str());
        let last_live = v.get("last_live_rubric").and_then(|s| s.as_str()).unwrap_or("?");
        let officium = v.get("first_officium").and_then(|s| s.as_str()).unwrap_or("");
        let kind = v.get("kind").and_then(|s| s.as_str()).unwrap_or("");
        let supp_str = suppressed
            .map(|s| format!(", suppressed in {s}"))
            .unwrap_or_default();
        println!(
            "  {key}\n    {officium} ({kind})\n    added in {added}, last live in {last_live}{supp_str}"
        );
    }
}
