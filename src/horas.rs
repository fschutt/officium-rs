//! Breviary corpus loader and per-hour rendering scaffolding.
//!
//! Mirrors the Mass-side `missa.rs` pair: this module loads the
//! upstream Breviary corpus (Tempora / Sancti / Commune horas files +
//! Ordinarium hour skeletons + Psalterium index + per-psalm bodies),
//! and exposes accessors against which `compute_office_hour` renders
//! the hour.
//!
//! B1 shipped the data layer.
//!
//! B2 adds [`compute_office_hour`]: walk an `Ordinarium/<HourName>`
//! template, expand `&MacroName` references against
//! `Psalterium/Common/Prayers`, and emit a structured `Vec<RenderedLine>`
//! with section slots that B3 will fill from the per-day Tempora /
//! Sancti propers.
//!
//! Source-of-truth: `vendor/divinum-officium/web/cgi-bin/horas/`
//! (entry `horas.pl`, walker `specials.pl`, per-hour helpers under
//! `specials/`). The data-extraction script is
//! `data/build_horas_json.py`.

use std::collections::HashMap;
use std::sync::OnceLock;

pub use crate::data_types::{HorasFile, OrdoLine, PsalmFile};
pub use crate::ordo::RenderedLine;

static HORAS_BR: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/horas_latin.postcard.br"));
static PSALMS_BR: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/psalms_latin.postcard.br"));

static HORAS: OnceLock<HashMap<String, HorasFile>> = OnceLock::new();
static PSALMS: OnceLock<HashMap<String, PsalmFile>> = OnceLock::new();

fn horas_corpus() -> &'static HashMap<String, HorasFile> {
    HORAS.get_or_init(|| {
        let pc = crate::embed::decompress(HORAS_BR);
        postcard::from_bytes(&pc).unwrap_or_default()
    })
}

fn psalm_corpus() -> &'static HashMap<String, PsalmFile> {
    PSALMS.get_or_init(|| {
        let pc = crate::embed::decompress(PSALMS_BR);
        postcard::from_bytes(&pc).unwrap_or_default()
    })
}

/// Look up a horas file by key. Examples:
///   * `Tempora/Pasc1-0`
///   * `Sancti/05-02`
///   * `Commune/C4-1`
///   * `Ordinarium/Vespera`
///   * `Psalterium/Psalmi/major`
///   * `Psalterium/Special/Major`
///   * `Psalterium/Invitatorium`
///   * `Psalterium/Common/Prayers`
pub fn lookup(key: &str) -> Option<&'static HorasFile> {
    horas_corpus().get(key)
}

/// Look up a section body inside a horas file. Tries the bare section
/// name first; if that miss, scans for any rubric-tagged variant
/// (`Hymnus Vespera (sed rubrica monastica)`) and returns the first
/// match. The rubric-aware selector lands in B2 — for now this is
/// section-name-first.
pub fn section<'a>(file: &'a HorasFile, name: &str) -> Option<&'a str> {
    file.sections.get(name).map(String::as_str)
}

/// Look up a psalm by upstream stem (`Psalm1` … `Psalm150` plus
/// split forms `Psalm17a` etc.). Returns the Vulgate body by default
/// — caller passes `bea = true` for the Pius XII Bea revision under
/// `psalmvar`.
pub fn psalm(stem: &str, bea: bool) -> Option<&'static str> {
    let entry = psalm_corpus().get(stem)?;
    let body = if bea && !entry.latin_bea.is_empty() {
        &entry.latin_bea
    } else {
        &entry.latin
    };
    Some(body.as_str())
}

/// Iterator over every loaded horas file. Used by B5+ regression
/// harness to enumerate the corpus.
pub fn iter() -> impl Iterator<Item = (&'static String, &'static HorasFile)> {
    horas_corpus().iter()
}

// ─── Hour walker (B2) ────────────────────────────────────────────────

/// The 8 canonical Roman office hours, plus aliases used elsewhere
/// in the corpus. The string form matches the upstream filename
/// stems under `horas/Ordinarium/`.
pub const HOUR_VESPERA: &str = "Vespera";
pub const HOUR_LAUDES: &str = "Laudes";
pub const HOUR_PRIMA: &str = "Prima";
pub const HOUR_MINOR: &str = "Minor";
pub const HOUR_MATUTINUM: &str = "Matutinum";
pub const HOUR_COMPLETORIUM: &str = "Completorium";

/// Inputs for [`compute_office_hour`]. B2 ships with `date` /
/// `rubric` / `hour` only — `solemn` and `rubrics` will become live
/// in B3 when the per-day proper splicing lands.
#[derive(Debug, Clone)]
pub struct OfficeArgs<'a> {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub rubric: crate::core::Rubric,
    /// Ordinarium hour stem — `Vespera`, `Laudes`, `Prima`, `Minor`,
    /// `Matutinum`, `Completorium`.
    pub hour: &'a str,
    /// User toggle: when off, level-1 rubric notes are suppressed
    /// (mirrors `propers.pl` line 107). Defaults to true.
    pub rubrics: bool,
}

/// Walk the requested Ordinarium hour template and emit a structured
/// list of [`RenderedLine`]s.
///
/// **B2 scope:** macros from `Psalterium/Common/Prayers` are inlined
/// (`&Deus_in_adjutorium`, `&Alleluia`, `&Dominus_vobiscum`,
/// `&Benedicamus_Domino`, `&Divinum_auxilium`). Section headings
/// (`#Psalmi`, `#Capitulum Hymnus Versus`, `#Canticum: Magnificat`,
/// `#Oratio`, `#Conclusio`, …) are emitted as `Section { label }`
/// slot markers. Plain-text lines are passed through verbatim — the
/// rubric-conditional `(sed rubrica X omittitur)` directives are not
/// yet evaluated; that lands in B3.
///
/// On unknown hour stem returns an empty vec.
pub fn compute_office_hour(args: &OfficeArgs<'_>) -> Vec<RenderedLine> {
    let key = format!("Ordinarium/{}", args.hour);
    let file = match lookup(&key) {
        Some(f) => f,
        None => return Vec::new(),
    };
    let prayers_file = lookup("Psalterium/Common/Prayers");
    let mut out = Vec::with_capacity(file.template.len());

    for line in &file.template {
        match line.kind.as_str() {
            "blank" => {}
            "section" => {
                if let Some(label) = &line.label {
                    out.push(RenderedLine::Section { label: label.clone() });
                }
            }
            "rubric" => {
                let level = line.level.unwrap_or(1);
                if level == 1 && !args.rubrics {
                    continue;
                }
                if let Some(body) = &line.body {
                    out.push(RenderedLine::Rubric { body: body.clone(), level });
                }
            }
            "spoken" => {
                if let (Some(role), Some(body)) = (&line.role, &line.body) {
                    out.push(RenderedLine::Spoken {
                        role: role.clone(),
                        body: body.clone(),
                    });
                }
            }
            "plain" => {
                if let Some(body) = &line.body {
                    out.push(RenderedLine::Plain { body: body.clone() });
                }
            }
            "macro" => {
                if let Some(name) = &line.name {
                    let body = lookup_horas_macro(prayers_file, name)
                        .unwrap_or("")
                        .to_string();
                    out.push(RenderedLine::Macro {
                        name: name.clone(),
                        body,
                    });
                }
            }
            "proper" | "hook" => {
                // B3+ wiring. Emit a slot marker so the slot is visible.
                if let Some(name) = &line.name {
                    out.push(RenderedLine::Proper { section: name.clone() });
                }
            }
            _ => {}
        }
    }
    out
}

/// Resolve `&Macro_With_Underscores` against the Breviary
/// `Psalterium/Common/Prayers` section map.
///
/// The upstream Perl walker treats most macros as a 1:1 underscore→
/// space mapping (`&Deus_in_adjutorium` → `[Deus in adjutorium]`).
/// A handful of names are ScriptFuncs in `horasscripts.pl` that
/// derive their body from a different base prayer — most importantly
/// `Dominus_vobiscum` returns selected lines of the `[Dominus]`
/// prayer based on priest/preces state. For B2 we approximate by
/// falling back to the first underscore-separated token if the
/// direct mapping misses; B3+ will refine to mirror the ScriptFunc
/// branch logic.
fn lookup_horas_macro<'a>(prayers: Option<&'a HorasFile>, name: &str) -> Option<&'a str> {
    let prayers = prayers?;
    let key = name.replace('_', " ");
    if let Some(body) = prayers.sections.get(&key) {
        return Some(body.as_str());
    }
    // Fallback: first token (`Dominus_vobiscum` → `Dominus`).
    let head = name.split('_').next().unwrap_or(name);
    prayers.sections.get(head).map(String::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corpus_loads_some_horas_files() {
        let n = horas_corpus().len();
        // B1 baseline: ~1,200 keys after the upstream tree is walked.
        // If this drops to 0 the embedded blob is the fallback empty
        // corpus — a build-time signal that data/build_horas_json.py
        // wasn't run.
        assert!(n > 1000, "horas corpus suspiciously small: {n} keys");
    }

    #[test]
    fn ordinarium_vespera_loads() {
        let f = lookup("Ordinarium/Vespera").expect("Ordinarium/Vespera missing");
        // Ordinarium files carry a `template`, not `sections`.
        assert!(!f.template.is_empty(), "Vespera ordinarium template empty");
        // Spot-check that a Magnificat insertion point exists.
        let has_magnificat = f.template.iter().any(|l| {
            l.kind == "section" && l.label.as_deref().map_or(false, |x| x.contains("Magnificat"))
        });
        assert!(has_magnificat, "Magnificat section missing from Vespera template");
    }

    #[test]
    fn psalm_1_has_latin_body() {
        let body = psalm("Psalm1", false).expect("Psalm1 missing");
        // Body uses the accented form `Beátus`. Check on a stem
        // unaffected by Latin diacritics.
        assert!(body.contains("Beátus vir") || body.contains("vir, qui non"),
            "Psalm 1 body unexpected: {}", &body[..body.len().min(120)]);
    }

    #[test]
    fn sancti_athanasius_has_lectio4() {
        let f = lookup("Sancti/05-02").expect("Sancti/05-02 missing");
        assert!(section(f, "Lectio4").is_some(), "Lectio4 missing in 05-02");
    }

    fn vespera_args(year: i32, month: u32, day: u32) -> OfficeArgs<'static> {
        OfficeArgs {
            year,
            month,
            day,
            rubric: crate::core::Rubric::Tridentine1570,
            hour: HOUR_VESPERA,
            rubrics: true,
        }
    }

    #[test]
    fn compute_office_hour_vespera_emits_walker_skeleton() {
        // 2026-05-04 — May 4th, today (per current-date context).
        let lines = compute_office_hour(&vespera_args(2026, 5, 4));
        assert!(!lines.is_empty(), "Vespera walker emitted nothing");

        // Every canonical Vespera section heading is present as a slot.
        let sections: Vec<&str> = lines
            .iter()
            .filter_map(|l| match l {
                RenderedLine::Section { label } => Some(label.as_str()),
                _ => None,
            })
            .collect();
        for want in [
            "Incipit",
            "Psalmi",
            "Canticum: Magnificat",
            "Oratio",
            "Conclusio",
        ] {
            assert!(
                sections.iter().any(|s| *s == want),
                "Vespera missing section slot {want}; got {sections:?}"
            );
        }

        // `&Deus_in_adjutorium` macro must expand to the full versicle
        // body from Psalterium/Common/Prayers.
        let deus = lines
            .iter()
            .find_map(|l| match l {
                RenderedLine::Macro { name, body } if name == "Deus_in_adjutorium" => Some(body),
                _ => None,
            })
            .expect("Deus_in_adjutorium macro missing");
        assert!(
            deus.contains("adjutórium meum inténde"),
            "Deus_in_adjutorium body not resolved: {deus:?}"
        );
        assert!(
            deus.contains("Glória Patri"),
            "Deus_in_adjutorium missing Gloria Patri tag"
        );

        // `&Benedicamus_Domino` and `&Dominus_vobiscum` resolve too.
        let names: Vec<&str> = lines
            .iter()
            .filter_map(|l| match l {
                RenderedLine::Macro { name, body } if !body.is_empty() => Some(name.as_str()),
                _ => None,
            })
            .collect();
        for want in ["Deus_in_adjutorium", "Alleluia", "Dominus_vobiscum", "Benedicamus_Domino"] {
            assert!(
                names.contains(&want),
                "macro {want} did not resolve; got {names:?}"
            );
        }
    }

    #[test]
    fn compute_office_hour_vespera_christmas_renders() {
        // Smoke-test on Christmas — same Vespera template; per-day
        // proper splicing arrives in B3.
        let lines = compute_office_hour(&vespera_args(2026, 12, 25));
        assert!(!lines.is_empty(), "Christmas Vespera walker empty");
        let n_sections = lines
            .iter()
            .filter(|l| matches!(l, RenderedLine::Section { .. }))
            .count();
        assert!(n_sections >= 5, "Christmas Vespera: only {n_sections} section slots emitted");
    }

    #[test]
    fn compute_office_hour_unknown_hour_returns_empty() {
        let args = OfficeArgs {
            year: 2026,
            month: 5,
            day: 4,
            rubric: crate::core::Rubric::Tridentine1570,
            hour: "NotAnHour",
            rubrics: true,
        };
        assert!(compute_office_hour(&args).is_empty());
    }
}
