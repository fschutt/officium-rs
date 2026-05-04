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

use crate::core::{Date, Locale, OfficeInput, ProperBlock, Rubric};
use crate::corpus::{BundledCorpus, Corpus};
use crate::mass::mass_propers;
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

/// Compute the full Mass propers + Ordinary toggles for a given date
/// + rubric. Returns a JSON string with both the resolved propers
/// (Latin bodies of Introitus / Oratio / Lectio / Graduale / Tractus
/// / Sequentia / Evangelium / Offertorium / Secreta / Prefatio /
/// Communio / Postcommunio + commemorations) AND the rules the
/// renderer needs to assemble the Ordinary correctly:
/// `gloria`, `credo`, `prefatio_name`, `season`, `color`, etc.
///
/// Returns shape:
/// ```json
/// {
///   "office": { winner, color, season, rank, rubric, commemorations },
///   "propers": {
///     "introitus":   {"latin": "...", "source": "Sancti/05-02", "via_commune": false},
///     "oratio":      {...},
///     "lectio":      {...},
///     "graduale":    {...},
///     "tractus":     null,
///     "sequentia":   null,
///     "evangelium":  {...},
///     "offertorium": {...},
///     "secreta":     {...},
///     "prefatio":    {...},
///     "communio":    {...},
///     "postcommunio":{...},
///     "commemorations": [...]
///   },
///   "rules": {
///     "gloria":        true,
///     "credo":         true,
///     "prefatio_name": "Apostolis",
///     "alleluia_seasonal": "paschal"
///   }
/// }
/// ```
#[wasm_bindgen]
pub fn compute_mass_json(year: i32, month: u32, day: u32, rubric: &str) -> String {
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
    let propers = mass_propers(&office, &corpus);

    // Pull the [Rule] from the winner mass file, parse the toggles we
    // need for the Ordinary renderer.
    let rules = pull_rules(&office.winner.render());

    let mut commems_json = Vec::<String>::new();
    if let Some(c) = &office.commemoratio {
        commems_json.push(format!("\"{}\"", json_escape(&c.render())));
    }

    let mut comm_blocks = Vec::<String>::new();
    for c in &propers.commemorations {
        comm_blocks.push(format!(
            r#"{{"source":"{}","oratio":{},"secreta":{},"postcommunio":{}}}"#,
            json_escape(&c.source.render()),
            block_json(c.oratio.as_ref()),
            block_json(c.secreta.as_ref()),
            block_json(c.postcommunio.as_ref()),
        ));
    }

    format!(
        r#"{{"office":{{"winner":"{winner}","color":"{color}","season":"{season}","rank":"{rank}","rubric":"{rubric}","commemorations":[{commems}]}},"propers":{{"introitus":{intr},"oratio":{ora},"lectio":{lec},"graduale":{grad},"tractus":{tr},"sequentia":{seq},"evangelium":{ev},"offertorium":{off},"secreta":{sec},"prefatio":{pre},"communio":{com},"postcommunio":{post},"commemorations":[{comm_blocks}]}},"rules":{rules}}}"#,
        winner = json_escape(&office.winner.render()),
        color = format!("{:?}", office.color),
        season = format!("{:?}", office.season),
        rank = json_escape(&office.rank.raw_label),
        rubric = format!("{:?}", office.rubric),
        commems = commems_json.join(","),
        intr = block_json(propers.introitus.as_ref()),
        ora = block_json(propers.oratio.as_ref()),
        lec = block_json(propers.lectio.as_ref()),
        grad = block_json(propers.graduale.as_ref()),
        tr = block_json(propers.tractus.as_ref()),
        seq = block_json(propers.sequentia.as_ref()),
        ev = block_json(propers.evangelium.as_ref()),
        off = block_json(propers.offertorium.as_ref()),
        sec = block_json(propers.secreta.as_ref()),
        pre = block_json(propers.prefatio.as_ref()),
        com = block_json(propers.communio.as_ref()),
        post = block_json(propers.postcommunio.as_ref()),
        comm_blocks = comm_blocks.join(","),
        rules = rules,
    )
}

fn block_json(b: Option<&ProperBlock>) -> String {
    match b {
        None => "null".to_string(),
        Some(b) => format!(
            r#"{{"latin":"{}","source":"{}","via_commune":{}}}"#,
            json_escape(&b.latin),
            json_escape(&b.source.render()),
            b.via_commune,
        ),
    }
}

/// Read winner [Rule] for the toggles the Mass-Ordinary renderer
/// needs. Returns a JSON object literal (no quotes around the
/// outermost braces — meant to be inlined).
fn pull_rules(winner_path: &str) -> String {
    let key = crate::core::FileKey::parse(winner_path);
    let Some(file) = BundledCorpus.mass_file(&key) else {
        return r#"{"gloria":true,"credo":false,"prefatio_name":"Communis"}"#.to_string();
    };
    let rule = file.sections.get("Rule").map(String::as_str).unwrap_or("");
    let lc = rule.to_lowercase();

    // Mirror Perl missa.pl: "no Gloria" / "no Credo" / "Credo" /
    // "Gloria" / "Prefatio=<Name>"; default Gloria=true, Credo=false.
    let gloria = !lc.contains("no gloria");
    let credo = lc.contains("credo");
    // "no Credo" should override "Credo" — check explicitly.
    let credo = credo && !lc.contains("no credo");

    let mut prefatio = "Communis".to_string();
    for line in rule.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("Prefatio=") {
            prefatio = rest.split_whitespace().next().unwrap_or("Communis").to_string();
            break;
        }
    }

    format!(
        r#"{{"gloria":{},"credo":{},"prefatio_name":"{}"}}"#,
        gloria,
        credo,
        json_escape(&prefatio),
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
