//! Data-access boundary for the rubric core.
//!
//! The pure functions in `occurrence`, `precedence`, and `mass`
//! consult `trait Corpus`; `BundledCorpus` wraps the JSON shipped
//! under `md2json2/data/`. Keeping data access behind a trait means
//! the core stays I/O-free and unit tests can swap in a `MockCorpus`
//! that returns hand-rolled rows for fault-line dates.
//!
//! The actual data types (`MassFile`, `SanctiEntry`, `KalendariaEntry`)
//! live where they always have — in `missa`, `sancti`, and
//! `kalendaria` respectively. This module just re-exposes them under
//! a single trait.
//!
//! Phase 1 ships the trait shape and a `BundledCorpus` whose methods
//! `todo!()` out. Phase 4 wires the real bodies through to the
//! existing `OnceLock` data once `compute_office()` actually needs
//! them.

use crate::divinum_officium::core::{FileKey, Rubric};
use crate::divinum_officium::kalendaria::KalendariaEntry;
use crate::divinum_officium::missa::MassFile;
use crate::divinum_officium::sancti::SanctiEntry;

/// What the kalendaria diff says about `(month, day, rubric)`.
/// Distinct from `kalendaria::Resolution` (which already folds in
/// the Sancti default fall-through) — this one is the *raw* override
/// answer, with the fall-through done by the consumer.
#[derive(Debug)]
pub enum KalendariaResolution<'a> {
    /// No diff entry for this date under this rubric — fall through
    /// to the default Sancti file.
    NoOverride,
    /// `XXXXX` marker: the date has *no* sanctoral office in this
    /// rubric (it is a feria of the temporal cycle).
    Suppressed,
    /// The diff supplies an override for this date.
    Override(&'a KalendariaEntry),
}

pub trait Corpus {
    /// Raw Sancti entries for `(month, day)`. Multiple when the
    /// upstream file carries rubric variants. Empty when no Sancti
    /// file ships for the date.
    fn sancti_entries(&self, month: u32, day: u32) -> &[SanctiEntry];

    /// What the kalendaria diff says for `(month, day)` under
    /// `rubric`. Each rubric layer has its own diff (1955.txt,
    /// 1960.txt, …); higher layers take precedence.
    fn kalendaria(&self, month: u32, day: u32, rubric: Rubric) -> KalendariaResolution<'_>;

    /// Mass-file body lookup by key — `Sancti/04-29`,
    /// `Tempora/Pasc3-0`, `Commune/C2a-1`, etc.
    fn mass_file(&self, key: &FileKey) -> Option<&MassFile>;
}

/// The production `Corpus` impl: thin shim over the existing
/// `OnceLock`-backed JSON loaders. Bodies land in Phase 4.
pub struct BundledCorpus;

impl Corpus for BundledCorpus {
    fn sancti_entries(&self, _month: u32, _day: u32) -> &[SanctiEntry] {
        todo!("Phase 4 — wire to data/sancti.json via a pub accessor in sancti.rs")
    }

    fn kalendaria(&self, _month: u32, _day: u32, _rubric: Rubric) -> KalendariaResolution<'_> {
        todo!("Phase 4 — wire to data/kalendaria_*.json (one per reform layer)")
    }

    fn mass_file(&self, _key: &FileKey) -> Option<&MassFile> {
        todo!("Phase 4 — wire to data/missa_latin.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Trait is object-safe — important so that tests can stub it via
    /// `Box<dyn Corpus>` if it suits them.
    #[test]
    fn corpus_trait_is_object_safe() {
        fn _accepts(_c: &dyn Corpus) {}
        // No instantiation needed; type-level check only.
    }
}
