//! WASM-bindgen surface.
//!
//! Browser/Node-callable entry points. Compiled only with
//! `--features wasm`; the rest of the crate has no `wasm-bindgen`
//! dependency.
//!
//! V1 ships `compute_office_json` only — given a date + rubric it
//! returns a JSON description of the winning office (file path, color,
//! season, rank, commemorations). Full Mass-propers body assembly is
//! deferred to V2 (see HISTORY.md).

use wasm_bindgen::prelude::*;

use crate::core::{Date, Locale, OfficeInput, Rubric};
use crate::corpus::BundledCorpus;
use crate::precedence::compute_office;

fn parse_rubric(s: &str) -> Option<Rubric> {
    Some(match s {
        "Tridentine1570" | "trid-1570" | "Tridentine - 1570" => Rubric::Tridentine1570,
        "Tridentine1910" | "trid-1910" | "Tridentine - 1910" => Rubric::Tridentine1910,
        "DivinoAfflatu1911" | "DivinoAfflatu" | "divino-afflatu" | "Divino Afflatu" => {
            Rubric::DivinoAfflatu1911
        }
        "Reduced1955" | "reduced-1955" | "Reduced - 1955" => Rubric::Reduced1955,
        "Rubrics1960" | "rubrics-1960" | "Rubrics 1960 - 1960" => Rubric::Rubrics1960,
        _ => return None,
    })
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str(r"\\"),
            '"' => out.push_str(r#"\""#),
            '\n' => out.push_str(r"\n"),
            '\r' => out.push_str(r"\r"),
            '\t' => out.push_str(r"\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

/// Compute the office for a given date + rubric.
///
/// Returns a JSON string with this shape:
/// ```json
/// {
///   "winner": "Sancti/05-02",
///   "color": "White",
///   "season": "PaschalTime",
///   "rank": "Duplex",
///   "rubric": "Rubrics1960",
///   "commemorations": []
/// }
/// ```
///
/// Errors return `{"error": "..."}`.
#[wasm_bindgen]
pub fn compute_office_json(year: i32, month: u32, day: u32, rubric: &str) -> String {
    let Some(rubric_enum) = parse_rubric(rubric) else {
        return r#"{"error":"unknown rubric"}"#.to_string();
    };
    let input = OfficeInput {
        date: Date::new(year, month, day),
        rubric: rubric_enum,
        locale: Locale::Latin,
    };
    let corpus = BundledCorpus;
    let office = compute_office(&input, &corpus);

    let mut commems: Vec<String> = Vec::new();
    if let Some(c) = &office.commemoratio {
        commems.push(c.render());
    }

    let commems_json: Vec<String> = commems
        .iter()
        .map(|p| format!("\"{}\"", json_escape(p)))
        .collect();

    format!(
        r#"{{"winner":"{}","color":"{}","season":"{}","rank":"{}","rubric":"{}","commemorations":[{}]}}"#,
        json_escape(&office.winner.render()),
        format!("{:?}", office.color),
        format!("{:?}", office.season),
        json_escape(&office.rank.raw_label),
        format!("{:?}", office.rubric),
        commems_json.join(","),
    )
}

/// Returns the list of supported rubric slugs as a JSON array.
#[wasm_bindgen]
pub fn supported_rubrics() -> String {
    r#"["trid-1570","trid-1910","divino-afflatu","reduced-1955","rubrics-1960"]"#.to_string()
}

/// Returns the crate version (from Cargo.toml at compile time).
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
