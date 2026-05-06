//! Breviary corpus access — file loader and section accessors.
//!
//! After M1 of the migration plan (see `BREVIARY_PORT_PLAN.md §6`),
//! this module owns the actual postcard-blob deserialization for the
//! Office corpus and the per-psalm bodies. Today it is a forwarding
//! shim into the working [`crate::horas`] module so the public API
//! exists for downstream slices to type-check against.
//!
//! Mirrors upstream `DivinumOfficium::SetupString::setupstring` and
//! `officestring` (`vendor/.../SetupString.pl:534-720`) at the lookup
//! layer; the per-section parsing lives in the build script
//! (`data/build_horas_json.py`) for now.

#[allow(unused_imports)]
use crate::data_types::{HorasFile, PsalmFile};

/// Look up a horas file by upstream key. Examples:
///
/// - `Tempora/Pasc1-0`
/// - `Sancti/05-02`
/// - `Commune/C4-1`
/// - `Ordinarium/Vespera`
/// - `Psalterium/Psalmi/major`
/// - `Psalterium/Common/Prayers`
///
/// **B10 status:** delegates to [`crate::horas::lookup`]. After M1 this
/// becomes the canonical implementation and `crate::horas::lookup` is
/// removed.
pub fn lookup(key: &str) -> Option<&'static HorasFile> {
    crate::horas::lookup(key)
}

/// Look up a section body inside a horas file by exact section name.
/// Mirrors upstream `$file{Section}` lookup; rubric-tag fallback
/// (`Hymnus Vespera (sed rubrica monastica)`) lands in B10 alongside
/// the runtime conditional evaluator.
pub fn section<'a>(file: &'a HorasFile, name: &str) -> Option<&'a str> {
    crate::horas::section(file, name)
}

/// Look up a psalm body by upstream stem (`Psalm1` … `Psalm150` plus
/// split forms `Psalm17a` etc.). When `bea` is true the Pius-XII Bea
/// revision is returned in preference to the Vulgate.
///
/// Callers should pass `bea = office_input.psalmvar` (the runtime
/// config field on `OfficeInput`, see `BREVIARY_PORT_PLAN.md §7.2`).
/// **Not** a Cargo feature flag — the same compiled binary serves
/// both Vulgate and Bea text per call.
pub fn psalm(stem: &str, bea: bool) -> Option<&'static str> {
    crate::horas::psalm(stem, bea)
}

/// Iterate over every loaded horas file. Used by regression / dump
/// tooling to enumerate the corpus.
pub fn iter() -> impl Iterator<Item = (&'static String, &'static HorasFile)> {
    crate::horas::iter()
}

/// Resolve a section, chasing one `@Path` or `@Path:OtherSection`
/// reference. Mirrors the upstream `setupstring` whole-section
/// inclusion. **Status:** stub — the working 1-hop implementation is
/// in `crate::horas::expand_at_redirect`. The full multi-hop variant
/// with `:s/PAT/REPL/` substitutions ships in B10 / `crate::setupstring`.
pub fn resolve_section(_file: &HorasFile, _section: &str) -> Option<(String, String)> {
    // TODO(B10): port `SetupString.pl::setupstring` lines 534-720
    // multi-hop inclusion, with `:in N loco s/PAT/REPL/` substitution.
    unimplemented!("phase B10: setupstring multi-hop @-redirect")
}
