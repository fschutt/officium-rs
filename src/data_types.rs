//! Data-file struct definitions, shared between the runtime lib and
//! the build script.
//!
//! `build.rs` pulls this file in via `#[path = "src/data_types.rs"]
//! mod data_types;` so that JSON → postcard transcoding at build time
//! uses the *same* struct shape the lib parses at runtime. Everything
//! here must be self-contained — no `crate::` references — so that
//! `build.rs`'s parallel compilation succeeds without a lib build.
//!
//! Each struct derives both `Serialize` (for postcard encoding in
//! build.rs) and `Deserialize` (for postcard decoding at lib runtime,
//! and for serde_json decoding from the source `.json` files in
//! build.rs).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─── Sancti corpus (`data/sancti.json`) ──────────────────────────────

/// One entry in `data/sancti.json` under a `MM-DD` key.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SanctiEntry {
    pub rubric: String,
    pub name: String,
    pub rank_class: String,
    pub rank_num: Option<f32>,
    pub commune: String,
}

// ─── Kalendaria 1962 (`data/kalendaria_1962.json`) ───────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KalendariaFeast {
    pub name: String,
    pub rank_num: Option<f32>,
    pub sancti_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KalendariaEntry {
    pub main: KalendariaFeast,
    pub commemorations: Vec<KalendariaFeast>,
}

// ─── Kalendaria by rubric (`data/kalendaria_by_rubric.json`) ─────────

/// One Sancti cell within a layer's resolved kalendar — `main` or
/// `commemoratio` of a date.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Cell {
    pub stem: String,
    pub officium: String,
    pub rank: String,
    #[serde(default)]
    pub rank_label: String,
    pub kind: String,
}

// ─── Mass Ordinary template (`data/ordo_latin.json`) ────────────────

/// One line of an Ordo template, plus its conditional guard. Mirrors
/// the upstream Perl walker's per-line decision in
/// `propers.pl::specials()` — the guard is consulted at render time
/// against the active `(solemn, defunctorum)` mode and the line is
/// either emitted, skipped, or — for hooks — dispatched to a
/// callback.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct OrdoLine {
    /// `Some("S"|"R"|"D"|"nD"|"SnD"|"RnD")` for flag guards, or
    /// `Some("&hookname")` for hook guards (e.g. `&CheckBlessing`).
    /// `None` means unconditional.  See `crate::ordo::Mode::passes_guard`.
    ///
    /// Note: postcard is a positional binary format — it always writes
    /// the discriminant byte for `Option` regardless of whether the
    /// field is present. We can't use `skip_serializing_if` here without
    /// breaking the build-time → runtime round-trip.
    #[serde(default)]
    pub guard: Option<String>,
    pub kind: String,
    /// Body for `plain`/`spoken`/`rubric`/`hook` (when hooks carry
    /// inline rubric text).
    #[serde(default)]
    pub body: Option<String>,
    /// Speaker tag for `spoken`: V/R/S/M/D/C/J.
    #[serde(default)]
    pub role: Option<String>,
    /// Section heading text for `section`.
    #[serde(default)]
    pub label: Option<String>,
    /// Rubric italic level (1, 2, 3) or omitted-comment marker (21, 22).
    #[serde(default)]
    pub level: Option<u8>,
    /// `&macroname` / `&propername` / `!&hookname` identifier.
    #[serde(default)]
    pub name: Option<String>,
    /// For `hook` lines that have inline rubric text after the hook
    /// name (e.g. `!*D In Missis Defunctorum dicit: …`).
    #[serde(default)]
    pub text: Option<String>,
}

/// Bundled Ordo corpus — all per-cursus templates + the macro
/// dictionary + the preface dictionary. Built from upstream
/// `Ordo/Ordo*.txt` + `Ordo/Prayers.txt` + `Ordo/Prefationes.txt` by
/// `data/build_ordo_json.py`.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct OrdoCorpus {
    /// Cursus name → ordered list of `OrdoLine`s. Keys: `Ordo`,
    /// `Ordo67`, `OrdoN`, `OrdoA`, `OrdoM`, `OrdoOP`, `OrdoS`.
    pub templates: HashMap<String, Vec<OrdoLine>>,
    /// `&MacroName` lookup — the static prayer texts referenced by
    /// `kind: "macro"` lines (e.g. `&Confiteor`, `&IteMissa`,
    /// `&DominusVobiscum`).
    pub prayers: HashMap<String, String>,
    /// Named preface bodies — `Prefationes.txt` keyed by name token
    /// (e.g. `Adv`, `Nat`, `Quad`, `Apostolis`, `Communis`).
    pub prefaces: HashMap<String, String>,
}

// ─── Mass corpus (`data/missa_latin.json`) ───────────────────────────

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct MassFile {
    #[serde(default)]
    pub officium: Option<String>,
    #[serde(default)]
    pub rank: Option<String>,
    #[serde(default)]
    pub rank_num: Option<f32>,
    #[serde(default)]
    pub rank_num_1570: Option<f32>,
    #[serde(default)]
    pub commune: Option<String>,
    #[serde(default)]
    pub commune_1570: Option<String>,
    #[serde(default)]
    pub officium_1906: Option<String>,
    #[serde(default)]
    pub rank_1906: Option<String>,
    #[serde(default)]
    pub rank_num_1906: Option<f32>,
    #[serde(default)]
    pub commune_1906: Option<String>,
    #[serde(default)]
    pub officium_sp: Option<String>,
    #[serde(default)]
    pub rank_sp: Option<String>,
    #[serde(default)]
    pub rank_num_sp: Option<f32>,
    #[serde(default)]
    pub commune_sp: Option<String>,
    #[serde(default)]
    pub officium_1955: Option<String>,
    #[serde(default)]
    pub rank_1955: Option<String>,
    #[serde(default)]
    pub rank_num_1955: Option<f32>,
    #[serde(default)]
    pub commune_1955: Option<String>,
    #[serde(default)]
    pub officium_1960: Option<String>,
    #[serde(default)]
    pub rank_1960: Option<String>,
    #[serde(default)]
    pub rank_num_1960: Option<f32>,
    #[serde(default)]
    pub commune_1960: Option<String>,
    #[serde(default)]
    pub sections: HashMap<String, String>,
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub parent_1570: Option<String>,
    #[serde(default)]
    pub annotated_sections: Vec<String>,
    #[serde(default)]
    pub annotated_section_meta: HashMap<String, String>,
}

// ─── Breviary corpus (`data/horas_latin.json`) ───────────────────────

/// One file in the upstream Breviary corpus — Tempora / Sancti /
/// Commune / Ordinarium / Psalterium index. Mirrors the Mass
/// `MassFile` shape but trimmed: no per-rubric metadata variants
/// (Breviary `[Rank]` is parsed at runtime by Rust against the active
/// rubric, not pre-baked into N copies).
///
/// Two payload shapes:
///   * **`sections`**: Tempora / Sancti / Commune / Psalterium files
///     use the `[Section] body` grammar — the resolver picks a
///     section by name (with optional rubric-tag fallback).
///   * **`template`**: Ordinarium hour skeletons use the
///     `#Section`/`&macro`/`$prayer`/`(sed rubrica X)` template
///     grammar shared with `Ordo/Ordo*.txt`; we reuse the same
///     [`OrdoLine`] shape so the Mass-side walker logic ports over.
///
/// Exactly one of the two is populated for any given file.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct HorasFile {
    #[serde(default)]
    pub sections: HashMap<String, String>,
    #[serde(default)]
    pub template: Vec<OrdoLine>,
}

// ─── Psalter corpus (`data/psalms_latin.json`) ───────────────────────

/// One psalm file — keyed by `Psalm{N}` or split form `Psalm17a`.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct PsalmFile {
    /// Vulgate Latin text (the default for all rubrics).
    #[serde(default)]
    pub latin: String,
    /// Pius XII / Bea revision — substituted under the `psalmvar`
    /// runtime flag. Empty when no Bea variant exists.
    #[serde(default)]
    pub latin_bea: String,
}
