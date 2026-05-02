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
