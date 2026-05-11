//! 1570 (Pius V) kalendar lookup — thin shim over
//! [`kalendaria_layers::Layer::Pius1570`].
//!
//! This module used to parse `data/kalendarium_1570.txt` directly;
//! that data is now sourced from the resolved per-rubric corpus
//! (`data/kalendaria_by_rubric.json`, built by
//! `data/build_canonization.py`) so all reform layers share the
//! same loader. The on-disk parse stays around as a regression
//! anchor (see the tests below) but the runtime path goes through
//! the layer module.
//!
//! `Entry1570` and `Feast1570` keep their existing shapes for
//! source-compat with `occurrence.rs`; new code should prefer the
//! `Layer`-keyed API in `kalendaria_layers`.

use crate::kalendaria_layers::{self, Layer};
use std::collections::BTreeMap;
use std::sync::OnceLock;

#[derive(Debug, Clone)]
pub struct Entry1570 {
    pub main: Feast1570,
    pub commemorations: Vec<Feast1570>,
}

#[derive(Debug, Clone)]
pub struct Feast1570 {
    /// Sancti-style file stem, e.g. `"01-23o"`, `"01-11cc"`, `"01-12t"`.
    /// The full `Sancti/<stem>` is the lookup key into the Mass corpus.
    pub stem: String,
    pub name: String,
    pub rank_num: f32,
}

/// Cached per-layer projection: `kalendaria_layers::Cell` lifted
/// into the legacy `Entry1570` shape so existing call sites stay
/// untouched. Built lazily on first lookup of each layer; one
/// `BTreeMap` per layer.
static PROJECTED: OnceLock<std::collections::HashMap<Layer, BTreeMap<(u32, u32), Entry1570>>> =
    OnceLock::new();

fn projected_for(layer: Layer) -> &'static BTreeMap<(u32, u32), Entry1570> {
    let all = PROJECTED.get_or_init(std::collections::HashMap::new);
    // Have to reach for unsafe-free interior mutability through
    // OnceLock-of-HashMap: we initialise once, then `entry_or_insert`
    // for any layer not yet projected. Using a static OnceLock + a
    // RefCell-like wrapper would be cleaner but adds a dep; for now
    // use a tiny static_per_layer constructor by precomputing all
    // seven on first call. Memory is ~50 KB total.
    static ALL_PROJECTED: OnceLock<std::collections::HashMap<Layer, BTreeMap<(u32, u32), Entry1570>>> =
        OnceLock::new();
    let _ = all; // silence
    ALL_PROJECTED.get_or_init(|| {
        let mut out = std::collections::HashMap::new();
        for layer in [
            Layer::Pius1570,
            Layer::LeoXIII1888,
            Layer::PiusX1906,
            Layer::PiusXI1939,
            Layer::PiusXIIPre1954,
            Layer::PiusXII1955,
            Layer::JohnXXIII1960,
        ] {
            out.insert(layer, project_layer(layer));
        }
        out
    }).get(&layer).expect("all seven layers projected")
}

fn project_layer(layer: Layer) -> BTreeMap<(u32, u32), Entry1570> {
    let mut out = BTreeMap::new();
    for mm in 1..=12u32 {
        let max_dd = if mm == 2 { 29 } else { 31 };
        for dd in 1..=max_dd {
            let Some(cells) = kalendaria_layers::lookup(layer, mm, dd) else {
                continue;
            };
            let main_cell = cells.iter().find(|c| c.is_main());
            let Some(main_cell) = main_cell else {
                continue;
            };
            let main = Feast1570 {
                stem: main_cell.stem.clone(),
                name: main_cell.officium.clone(),
                rank_num: main_cell.rank_num().unwrap_or(0.0),
            };
            let commemorations: Vec<Feast1570> = cells
                .iter()
                .filter(|c| !c.is_main())
                .map(|c| Feast1570 {
                    stem: c.stem.clone(),
                    name: c.officium.clone(),
                    rank_num: c.rank_num().unwrap_or(0.0),
                })
                .collect();
            out.insert(
                (mm, dd),
                Entry1570 {
                    main,
                    commemorations,
                },
            );
        }
    }
    out
}

/// Look up the 1570 entry for `(month, day)`. Returns `None` when the
/// kalendar table doesn't list this date — the consumer should fall
/// back to the temporal cycle (a feria).
pub fn lookup(month: u32, day: u32) -> Option<&'static Entry1570> {
    projected_for(Layer::Pius1570).get(&(month, day))
}

/// Layer-aware variant — same return shape, but reads the
/// `kalendaria_layers` projection for any of the seven supported
/// rubric layers. Used by reform-layer code paths so they don't
/// have to convert `kalendaria_layers::Cell` themselves.
pub fn lookup_for_layer(
    layer: Layer,
    month: u32,
    day: u32,
) -> Option<&'static Entry1570> {
    projected_for(layer).get(&(month, day))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_emerentiana_replaces_raymond() {
        // 01-23 in 1570 is Emerentiana (Simplex), file stem `01-23o`.
        // The post-1570 corpus has `Sancti/01-23` = Raymond of Penyafort
        // (instituted 1601); the 1570 kalendar redirects.
        let e = lookup(1, 23).expect("01-23 should exist");
        assert_eq!(e.main.stem, "01-23o");
        assert_eq!(e.main.rank_num, 1.0);
        assert!(e.main.name.contains("Emerentian"), "{}", e.main.name);
        assert!(e.commemorations.is_empty());
    }

    #[test]
    fn parse_octave_with_commemoration() {
        // 01-11 = Sexta die infra Oct Epi (Semiduplex 2) +
        // S. Hyginus comm (Simplex 1).
        let e = lookup(1, 11).expect("01-11 should exist");
        assert_eq!(e.main.stem, "01-11");
        assert_eq!(e.main.rank_num, 2.0);
        assert_eq!(e.commemorations.len(), 1);
        assert_eq!(e.commemorations[0].stem, "01-11cc");
        assert_eq!(e.commemorations[0].rank_num, 1.0);
    }

    #[test]
    fn nativity_bmv_octave_via_t1910_layer() {
        // T1910 inherits 1570's 09-15=09-15t redirect (Octave Day of
        // Nativity BMV) — neither 1888 nor 1906 overrides. Mirrors the
        // assertion in kalendaria_layers but exercises the
        // kalendarium_1570 wrapper used by occurrence.rs.
        let e = lookup_for_layer(Layer::PiusX1906, 9, 15);
        assert_eq!(e.map(|x| x.main.stem.as_str()), Some("09-15t"));
    }

    #[test]
    fn vigil_of_matthias_in_1570() {
        // 02-23 = Vigil of Matthias, stem `02-23o`, rank 1.5 (Vigilia).
        let e = lookup(2, 23).expect("02-23 should exist");
        assert_eq!(e.main.stem, "02-23o");
        assert_eq!(e.main.rank_num, 1.5);
    }

    #[test]
    fn ferial_date_returns_none() {
        // 04-29 (S. Petri Mart) is post-1570; the 1570 kalendar
        // doesn't list this date.
        assert!(lookup(4, 29).is_none());
    }

    #[test]
    fn parses_dominical_letter_files() {
        // 01-12 = Dominica infra Oct Epi (the Sunday-stem variant
        // for years where Jan 12 is Sunday).
        let e = lookup(1, 12).expect("01-12 should exist");
        assert_eq!(e.main.stem, "01-12t");
    }
}
