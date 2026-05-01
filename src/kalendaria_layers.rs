//! Year-aware kalendar layer lookup.
//!
//! The Divinum Officium upstream ships seven `Tabulae/Kalendaria/X.txt`
//! files that form a chain of SUPERSEDING DIFFS over the 1570 baseline:
//!
//!   1570 → 1888 → 1906 → 1939 → 1954 → 1955 → 1960
//!
//! `data/build_canonization.py` walks that chain and writes
//! `data/kalendaria_by_rubric.json` containing the *resolved* table
//! at each point — i.e. the full per-mmdd assignment after applying
//! every layer up to and including the keyed one. This module loads
//! that JSON via `include_str!` and exposes:
//!
//!   * [`Layer`] — typed enum of the seven layer keys.
//!   * [`layer_for_year(year)`] — maps a calendar year to its active
//!     layer (e.g. 1700 → `Pius1570`, 1900 → `LeoXIII1888`,
//!     1930 → `PiusX1906`, 1965 → `JohnXXIII1960`).
//!   * [`lookup(layer, month, day)`] — typed entry lookup with cells
//!     converted to native Rust ranks (so callers don't deal with
//!     Perl's `;;Vigilia;;1.5` string form).
//!
//! Adding a new reform layer is a data-only change: extend the
//! `Tabulae/Kalendaria/<NAME>.txt` chain, re-run `build_canonization.py`,
//! and add a `Layer` variant. No Sancti / occurrence / precedence
//! code change is required for a pure kalendar diff (rubric-rule
//! changes still need rules-side wiring).

use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

/// One Sancti cell within a layer's resolved kalendar — `main` or
/// `commemoratio` of a date.
#[derive(Debug, Clone, Deserialize)]
pub struct Cell {
    /// Sancti file stem (`01-01`, `02-23o`, `08-09cc`, …). The
    /// `Sancti/<stem>` is the lookup key into the Mass / Office corpus.
    pub stem: String,
    /// Officium label as it appears in the upstream kalendar
    /// (e.g. `"In Circumcisione Domini"`). When the same stem appears
    /// in multiple layers with different officium strings, this is
    /// the layer-specific value.
    pub officium: String,
    /// Numeric rank as a string (`"1"`..`"7"`, `"1.5"`). Native f32
    /// form is exposed via `Cell::rank_num()`.
    pub rank: String,
    /// Human-readable rank class label (`"Simplex"`, `"Vigilia"`,
    /// `"Duplex II classis"`, …) — empty when no label is known.
    #[serde(default)]
    pub rank_label: String,
    /// `"main"` or `"commemoratio"`.
    pub kind: String,
}

impl Cell {
    /// Convert the rank string to f32 (`Some` when parseable,
    /// `None` for blank). Mirrors the Sancti-corpus rank convention
    /// (1 = Simplex, 1.5 = Vigilia, 2 = Semiduplex, 3 = Duplex,
    /// 4 = Duplex majus, 5 = II classis, 6 = I classis, 7 = privileg).
    pub fn rank_num(&self) -> Option<f32> {
        self.rank.trim().parse().ok()
    }

    /// True for the main-feast cell of a date; false for any
    /// commemoration appended after `~`.
    pub fn is_main(&self) -> bool {
        self.kind == "main"
    }
}

/// One day's resolved cells under a specific layer. The first cell
/// is the main feast; subsequent cells are commemorations.
pub type Entry = Vec<Cell>;

/// The seven kalendar layers shipped by upstream's
/// `Tabulae/Kalendaria/`. Each is a *cumulative resolved* layer —
/// i.e. the result of applying every diff up to and including this
/// one onto the 1570 baseline. Variants are ordered chronologically
/// for `cmp` comparisons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Layer {
    /// Pius V baseline (`1570.txt`). Active 1570-1887.
    Pius1570,
    /// Leo XIII / Pius IX-era kalendar updates (`1888.txt`). Active
    /// 1888-1905.
    LeoXIII1888,
    /// Pius X early (pre-Divino-Afflatu) reforms (`1906.txt`).
    /// Active 1906-1938.
    PiusX1906,
    /// Pius XI updates (Christ the King 1925, etc.) (`1939.txt`).
    /// Active 1939-1953.
    PiusXI1939,
    /// Pius XII pre-Reduced kalendar (`1954.txt`). Active 1954-Aug 1955.
    PiusXIIPre1954,
    /// Pius XII Reduced (`Cum nostra hac aetate`, `1955.txt`).
    /// Active Aug 1955-1959.
    PiusXII1955,
    /// John XXIII Rubrics + 1962 typical edition (`1960.txt`).
    /// Active 1960 onward.
    JohnXXIII1960,
}

impl Layer {
    /// JSON key — matches the top-level keys in
    /// `data/kalendaria_by_rubric.json`.
    pub fn key(self) -> &'static str {
        match self {
            Self::Pius1570 => "1570",
            Self::LeoXIII1888 => "1888",
            Self::PiusX1906 => "1906",
            Self::PiusXI1939 => "1939",
            Self::PiusXIIPre1954 => "1954",
            Self::PiusXII1955 => "1955",
            Self::JohnXXIII1960 => "1960",
        }
    }
}

/// Map a calendar year to its active kalendar layer. The breakpoints
/// follow the publication years of the upstream `Tabulae/Kalendaria/`
/// files. For mid-year reforms (Pius XII Reduced was promulgated
/// March 1955, effective 1 Jan 1956) this is approximate — the
/// breakpoint stays at 1955 for simplicity, which matches upstream's
/// per-year directorium key.
pub fn layer_for_year(year: i32) -> Layer {
    match year {
        ..=1887 => Layer::Pius1570,
        1888..=1905 => Layer::LeoXIII1888,
        1906..=1938 => Layer::PiusX1906,
        1939..=1953 => Layer::PiusXI1939,
        1954 => Layer::PiusXIIPre1954,
        1955..=1959 => Layer::PiusXII1955,
        _ => Layer::JohnXXIII1960,
    }
}

static KALENDARIA_JSON: &str = include_str!("../../data/kalendaria_by_rubric.json");
static PARSED: OnceLock<HashMap<String, HashMap<String, Entry>>> = OnceLock::new();

fn parsed() -> &'static HashMap<String, HashMap<String, Entry>> {
    PARSED.get_or_init(|| serde_json::from_str(KALENDARIA_JSON).unwrap_or_default())
}

/// Look up the resolved entry for `(month, day)` under `layer`.
/// Returns `None` when the layer's kalendar doesn't list the date —
/// the consumer should treat the date as ferial (no sanctoral
/// office) under that layer.
pub fn lookup(layer: Layer, month: u32, day: u32) -> Option<&'static Entry> {
    let mm_dd = format!("{month:02}-{day:02}");
    parsed().get(layer.key())?.get(&mm_dd)
}

/// Convenience: look up under the layer that's active in `year`.
pub fn lookup_for_year(year: i32, month: u32, day: u32) -> Option<&'static Entry> {
    lookup(layer_for_year(year), month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn year_to_layer_breakpoints() {
        assert_eq!(layer_for_year(1700), Layer::Pius1570);
        assert_eq!(layer_for_year(1887), Layer::Pius1570);
        assert_eq!(layer_for_year(1888), Layer::LeoXIII1888);
        assert_eq!(layer_for_year(1900), Layer::LeoXIII1888);
        assert_eq!(layer_for_year(1906), Layer::PiusX1906);
        assert_eq!(layer_for_year(1925), Layer::PiusX1906);
        assert_eq!(layer_for_year(1939), Layer::PiusXI1939);
        assert_eq!(layer_for_year(1954), Layer::PiusXIIPre1954);
        assert_eq!(layer_for_year(1955), Layer::PiusXII1955);
        assert_eq!(layer_for_year(1959), Layer::PiusXII1955);
        assert_eq!(layer_for_year(1960), Layer::JohnXXIII1960);
        assert_eq!(layer_for_year(2026), Layer::JohnXXIII1960);
    }

    #[test]
    fn circumcision_present_in_every_layer() {
        // 01-01 = In Circumcisione Domini lives in 1570 baseline and
        // never gets removed.
        for layer in [
            Layer::Pius1570,
            Layer::LeoXIII1888,
            Layer::PiusX1906,
            Layer::PiusXI1939,
            Layer::PiusXIIPre1954,
            Layer::PiusXII1955,
            Layer::JohnXXIII1960,
        ] {
            let entry = lookup(layer, 1, 1).expect("01-01 must exist");
            assert!(entry[0].officium.contains("Circumcisione"), "{layer:?}");
        }
    }

    #[test]
    fn vigil_of_all_saints_suppressed_in_1955() {
        // 10-31 was the Vigil of All Saints under 1570/1888/1906/1939/1954,
        // suppressed by the Pius XII Reduced reform (1955) and stays
        // suppressed under John XXIII (1960).
        assert!(lookup(Layer::PiusXIIPre1954, 10, 31).is_some());
        assert!(lookup(Layer::PiusXII1955, 10, 31).is_none());
        assert!(lookup(Layer::JohnXXIII1960, 10, 31).is_none());
    }

    #[test]
    fn joseph_the_worker_is_pius_xii_only() {
        // 05-01r = Joseph the Worker (Pius XII 1955). Doesn't exist
        // before 1955.
        assert!(lookup(Layer::PiusXIIPre1954, 5, 1).map(|e| e[0].stem.as_str()).unwrap_or("") != "05-01r");
        assert_eq!(
            lookup(Layer::PiusXII1955, 5, 1).map(|e| e[0].stem.as_str()),
            Some("05-01r"),
        );
    }

    #[test]
    fn cell_rank_num_parses_vigilia() {
        // 02-23o = Vigil of Matthias, rank 1.5.
        let e = lookup(Layer::Pius1570, 2, 23).unwrap();
        assert_eq!(e[0].stem, "02-23o");
        assert_eq!(e[0].rank_num(), Some(1.5));
    }

    #[test]
    fn ferial_date_returns_none() {
        // 04-29 (S. Petri Martyris is post-1570) — the 1570 layer
        // doesn't list this date.
        assert!(lookup(Layer::Pius1570, 4, 29).is_none());
    }
}
