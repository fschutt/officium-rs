//! Breviary (Divine Office) rendering — slice B10+ scaffolding.
//!
//! This module is the canonical home for Office-side rendering after
//! B10. The current working entry point is still
//! [`crate::horas::compute_office_hour`], which represents the B1–B7
//! parity that lit up `/wip/breviary` and the `breviary.html` demo.
//! B10 onwards is being scaffolded here, function by function, before
//! the contents of `src/horas.rs` are migrated in. See
//! [`docs/BREVIARY_PORT_PLAN.md`] for the migration sequencing.
//!
//! ## Layout
//!
//! Mirrors the upstream Perl tree under
//! `vendor/divinum-officium/web/cgi-bin/horas/`:
//!
//! | Rust | Perl |
//! |---|---|
//! | [`corpus`] | the file loaders (a slice of `SetupString.pl` plus the build-time JSON) |
//! | [`horas`] | `horas.pl` — top-level `horas($hora)` orchestrator |
//! | [`specials`] | `specials.pl` — the per-hour template walker |
//! | [`proprium`] | `specials.pl::getproprium` + commune fallback chain |
//! | [`concurrence`] | `horascommon.pl:810-1480` — first-Vespers split |
//! | [`setheadline`] | `horascommon.pl::setheadline` / `rankname` |
//! | [`gettempora`] | `horascommon.pl::gettempora` |
//! | [`papal`] | `horascommon.pl` papal-rule helpers |
//! | [`psalter`] | `specials/psalmi.pl` |
//! | [`antetpsalm`] | psalm/antiphon formatter |
//! | [`hymnus`] | `specials/hymni.pl` |
//! | [`capitulum`] | `specials/capitulis.pl` |
//! | [`canticum`] | `horas.pl::canticum` + `ant123_special` |
//! | [`oratio`] | `specials/orationes.pl` (the densest single chunk) |
//! | [`suffragium`] | Suffragium of All Saints (pre-1955) |
//! | [`dirge`] | `DivinumOfficium::Directorium::dirge` |
//! | [`preces`] | `specials/preces.pl` |
//! | [`prima`] | `specials/specprima.pl` (Prime is its own hour) |
//! | [`martyrologium`] | `specials/specprima.pl::martyrologium` + `luna` + `gregor` |
//! | [`matins`] | `specmatins.pl` (1857 LOC, the densest hour) |
//! | [`postprocess`] | `horas.pl` text-postprocessing helpers |
//! | [`triduum`] | gloria-omission predicates (`triduum_gloria_omitted`, `Septuagesima_vesp`) |
//! | [`monastic`] | placeholder — pre-Trident Monastic (out of scope for first parity pass) |
//! | [`altovadum`] | placeholder — Cistercian Altovadensis (out of scope for first parity pass) |
//!
//! ## Architectural rules (from `feedback_divinum_officium_port.md`)
//!
//! - **1570 baseline + composable reform layers.** Every renderer is
//!   pure over `(office: &OfficeOutput, hour: Hour, corpus: &dyn Corpus) -> Vec<RenderedLine>`.
//!   No `our $hora`/`@dayname`/`%winner` global thrash.
//! - **Pure functions, no globals.** Use `thread_local!` only when an
//!   upstream helper genuinely needs an active-rubric ambient (mirror
//!   the pattern from [`crate::mass`]).
//! - **DiPippo > Perl when they disagree.** Document divergences
//!   in `docs/UPSTREAM_WEIRDNESSES_BREVIARY.md` (created during B19/B20).
//! - **Gitignored Perl vendor.** The `vendor/divinum-officium/` tree
//!   is read-only reference; never commit edits to it.
//!
//! ## What's NOT here yet
//!
//! - Working bodies. Every public function in this tree is
//!   `unimplemented!("phase BNN")` until its slice lands.
//! - The contents of `src/horas.rs`. They stay in place and keep the
//!   B1–B7 path working until the migration described in
//!   `BREVIARY_PORT_PLAN.md §6` happens.

pub mod corpus;
pub mod horas;
pub mod specials;
pub mod proprium;
pub mod concurrence;
pub mod setheadline;
pub mod gettempora;
pub mod papal;
pub mod psalter;
pub mod antetpsalm;
pub mod hymnus;
pub mod capitulum;
pub mod canticum;
pub mod oratio;
pub mod suffragium;
pub mod dirge;
pub mod preces;
pub mod prima;
pub mod martyrologium;
pub mod matins;
pub mod postprocess;
pub mod triduum;
pub mod monastic;
pub mod altovadum;

// Convenience re-exports of the most commonly used types and entry
// points. Keep this short — the goal is `use officium_rs::breviary;`
// followed by `breviary::compute_office_hour(...)`.
pub use horas::{compute_office_hour, OfficeArgs, Hour};
pub use corpus::{lookup, psalm};
