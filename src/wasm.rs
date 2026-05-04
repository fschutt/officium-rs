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
use crate::ordo::{self, Mode, RenderArgs, RenderedLine};
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

/// Compute the full Mass — propers + rules + the rendered Mass
/// Ordinary as a flat list of typed lines. Drops every shred of
/// hardcoded Latin from the JS demo: this returns enough structure
/// for the renderer to walk and emit HTML against, with no per-cursus
/// Ordinary text living anywhere outside the upstream Perl corpus.
///
/// The returned `ordinary` array carries one of these line shapes:
/// ```json
///   {"k": "section",  "label": "Incipit"}
///   {"k": "rubric",   "body": "...", "level": 1}
///   {"k": "spoken",   "role": "V",   "body": "..."}
///   {"k": "plain",    "body": "..."}
///   {"k": "macro",    "name": "Confiteor", "body": "..."}
///   {"k": "proper",   "section": "introitus"}
///   {"k": "hook",     "hook": "Introibo", "message": "omit. psalm"}
/// ```
///
/// Mode flags:
///   * `solemn`     — solemn (sung) vs. low Mass; defaults `true`.
///   * `rubrics`    — emit level-1 italic rubrics? defaults `true`.
///   * `defunctorum` is auto-inferred from the office: true when the
///     winner path contains `Defunct` or `[Rule]` mentions a Requiem.
#[wasm_bindgen]
pub fn compute_mass_full(
    year: i32,
    month: u32,
    day: u32,
    rubric: &str,
    solemn: bool,
    rubrics: bool,
) -> String {
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
    let rules_obj = parse_rules(&office.winner.render());
    let dayname = derive_dayname(&office.winner);
    let defunctorum = is_defunctorum(&office.winner.render(), &rules_obj.rule_raw);

    let mode = Mode {
        solemn,
        defunctorum,
        dayofweek: weekday_of(office.date) as u8,
        dayname,
        rule_lc: rules_obj.rule_raw.to_lowercase(),
    };
    let template_name = ordo::template_name_for_rubric(rubric_enum);
    let args = RenderArgs {
        mode: &mode,
        gloria_active: rules_obj.gloria,
        credo_active: rules_obj.credo,
        rubrics,
        template_name,
    };
    let lines = ordo::render_mass(&args);

    let ordinary_json = render_lines_to_json(&lines);

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
        r#"{{"office":{{"winner":"{winner}","color":"{color}","season":"{season}","rank":"{rank}","rubric":"{rubric}","commemorations":[{commems}]}},"propers":{{"introitus":{intr},"oratio":{ora},"lectio":{lec},"graduale":{grad},"tractus":{tr},"sequentia":{seq},"evangelium":{ev},"offertorium":{off},"secreta":{sec},"prefatio":{pre},"communio":{com},"postcommunio":{post},"commemorations":[{comm_blocks}]}},"rules":{{"gloria":{gloria},"credo":{credo},"prefatio_name":"{pref_name}","solemn":{solemn},"defunctorum":{defunct}}},"ordinary":[{ordinary}]}}"#,
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
        gloria = rules_obj.gloria,
        credo = rules_obj.credo,
        pref_name = json_escape(&rules_obj.prefatio_name),
        solemn = solemn,
        defunct = defunctorum,
        ordinary = ordinary_json,
    )
}

/// Parsed-out [Rule] facets for the Ordinary renderer + JSON output.
struct ParsedRules {
    gloria: bool,
    credo: bool,
    prefatio_name: String,
    rule_raw: String,
}

fn parse_rules(winner_path: &str) -> ParsedRules {
    let key = crate::core::FileKey::parse(winner_path);
    let Some(file) = BundledCorpus.mass_file(&key) else {
        return ParsedRules {
            gloria: true,
            credo: false,
            prefatio_name: "Communis".to_string(),
            rule_raw: String::new(),
        };
    };
    let rule = file.sections.get("Rule").map(String::as_str).unwrap_or("");
    let lc = rule.to_lowercase();
    let gloria = !lc.contains("no gloria");
    let credo = lc.contains("credo") && !lc.contains("no credo");
    let mut prefatio_name = "Communis".to_string();
    for line in rule.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("Prefatio=") {
            prefatio_name = rest.split_whitespace().next().unwrap_or("Communis").to_string();
            break;
        }
    }
    ParsedRules {
        gloria,
        credo,
        prefatio_name,
        rule_raw: rule.to_string(),
    }
}

/// Infer Perl `$votive =~ /Defunct|C9/i` for the Ordinary renderer.
/// `Defunct` matches the Requiem files (`Sancti/11-02`, votive
/// Defunctorum), `C9` is the Cross commune.
///
/// We check three signals:
///   * the winner path itself (votive Defunctorum lives under
///     `Tempora/Defunct…` or similar);
///   * the `[Rank]` row (e.g. `In Commemoratione Omnium Fidelium
///     Defunctorum;;Duplex;;3;;ex C9`) — this catches All Souls;
///   * the `[Rule]` body for `Defunct` / `C9` mentions.
fn is_defunctorum(winner_path: &str, rule: &str) -> bool {
    let p_lc = winner_path.to_lowercase();
    let r_lc = rule.to_lowercase();
    if p_lc.contains("defunct") || r_lc.contains("defunct") || r_lc.contains("c9") {
        return true;
    }
    // Inspect the winner's parsed metadata — All Souls (`Sancti/11-02`)
    // carries `officium = "In Commemoratione Omnium Fidelium
    // Defunctorum"` and `commune = "C9"`. The [Rank] line is parsed
    // out into named fields by `build_missa_json.py`; we don't see it
    // as a section.
    let key = crate::core::FileKey::parse(winner_path);
    if let Some(file) = BundledCorpus.mass_file(&key) {
        let off_lc = file.officium.as_deref().unwrap_or("").to_lowercase();
        let comm_lc = file.commune.as_deref().unwrap_or("").to_lowercase();
        if off_lc.contains("defunct") || comm_lc == "c9" {
            return true;
        }
    }
    false
}

/// Pick the `dayname[0]` token used for hook predicates. Maps the
/// winner FileKey back to the Perl convention (`Adv1-0`, `Pasc3-0`,
/// `Quad6-5`). Sancti winners produce empty — the hooks that consult
/// dayname (Introibo / Vidiaquam) only fire on tempora-like seasons.
fn derive_dayname(winner: &crate::core::FileKey) -> String {
    use crate::core::FileCategory;
    match winner.category {
        FileCategory::Tempora => winner.stem.clone(),
        _ => String::new(),
    }
}

fn weekday_of(date: crate::core::Date) -> u32 {
    crate::date::day_of_week(date.day, date.month, date.year)
}

/// Encode a [`RenderedLine`] list as a JSON array body (no surrounding
/// brackets — caller wraps).
fn render_lines_to_json(lines: &[RenderedLine]) -> String {
    let mut buf = String::with_capacity(lines.len() * 64);
    for (i, l) in lines.iter().enumerate() {
        if i > 0 {
            buf.push(',');
        }
        match l {
            RenderedLine::Plain { body } => {
                buf.push_str(&format!(r#"{{"k":"plain","body":"{}"}}"#, json_escape(body)));
            }
            RenderedLine::Spoken { role, body } => {
                buf.push_str(&format!(
                    r#"{{"k":"spoken","role":"{}","body":"{}"}}"#,
                    json_escape(role),
                    json_escape(body),
                ));
            }
            RenderedLine::Rubric { body, level } => {
                buf.push_str(&format!(
                    r#"{{"k":"rubric","body":"{}","level":{}}}"#,
                    json_escape(body),
                    level,
                ));
            }
            RenderedLine::Section { label } => {
                buf.push_str(&format!(r#"{{"k":"section","label":"{}"}}"#, json_escape(label)));
            }
            RenderedLine::Macro { name, body } => {
                buf.push_str(&format!(
                    r#"{{"k":"macro","name":"{}","body":"{}"}}"#,
                    json_escape(name),
                    json_escape(body),
                ));
            }
            RenderedLine::Proper { section } => {
                buf.push_str(&format!(
                    r#"{{"k":"proper","section":"{}"}}"#,
                    json_escape(section),
                ));
            }
            RenderedLine::HookOmit { hook, message } => {
                buf.push_str(&format!(
                    r#"{{"k":"hook","hook":"{}","message":"{}"}}"#,
                    json_escape(hook),
                    json_escape(message),
                ));
            }
        }
    }
    buf
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_mass_full_emits_ordinary_for_easter_sunday() {
        // Pasc1-0 — Easter Sunday under DA. Should have Gloria,
        // emit propers + section headers + the Last Gospel block.
        let json = compute_mass_full(2026, 4, 5, "divino-afflatu", true, true);
        assert!(!json.contains("\"error\""), "got error JSON: {json}");
        assert!(json.contains("\"ordinary\":["), "missing ordinary array");
        // Spot-check the structural pieces. Note: Confiteor is inlined
        // as `spoken` lines in upstream Ordo.txt — not a `&Confiteor`
        // macro reference. The two real macro references are
        // `&DominusVobiscum` and `&Gloria`.
        assert!(json.contains("\"name\":\"Gloria\""), "missing Gloria macro reference");
        assert!(json.contains("\"section\":\"introitus\""), "missing introitus proper");
        assert!(json.contains("\"section\":\"Ultimaev\""), "missing last-gospel proper");
        assert!(json.contains("\"label\":\"Incipit\""), "missing Incipit section header");
        // The Confiteor body should appear as a spoken line. Body is
        // copied verbatim — no JSON unicode escaping for accented
        // characters since `json_escape` only escapes control bytes.
        assert!(json.contains("Confíteor Deo") || json.contains("Confiteor Deo"),
            "Confiteor body absent — first 1500 chars:\n{}", &json[..json.len().min(1500)]);
    }

    #[test]
    fn compute_mass_full_drops_leonine_under_solemn() {
        // Solemn Mass: `!*R` blocks (Leonine prayers) are dropped.
        let json = compute_mass_full(2026, 4, 5, "divino-afflatu", true, true);
        assert!(!json.contains("Leonis XIII"), "Leonine prayers should be solemn-skipped");
    }

    #[test]
    fn compute_mass_full_emits_leonine_under_low_mass() {
        // Low Mass: `!*R` blocks emitted.
        let json = compute_mass_full(2026, 4, 5, "divino-afflatu", false, true);
        assert!(json.contains("Leonis XIII"), "Leonine prayers should appear under low Mass");
    }
}
