//! Per-hour template walker — Office side.
//!
//! Mirror of `vendor/divinum-officium/web/cgi-bin/horas/specials.pl`:
//!
//! - `specials(\@s, $lang)` (lines 21-408) — the section-by-section
//!   walker that drives an Ordinarium template (`Matutinum.txt`,
//!   `Laudes.txt`, `Prima.txt`, `Minor.txt`, `Vespera.txt`,
//!   `Completorium.txt`) by dispatching each `#Section` heading to the
//!   right per-section helper (psalter / hymn / capitulum / canticum /
//!   oratio / preces / commemorations).
//! - `getproprium` (lines 443-521) — see [`crate::breviary::proprium`].
//! - `loadspecial` (line 769) — replaces the entire script with a
//!   `Special $hora` body when the day file carries one.
//! - `replaceNdot` (line 782) — saint-name placeholder substitution
//!   (already partially in `crate::horas::substitute_saint_name`).
//!
//! ## Architectural notes
//!
//! The Perl walker mutates a global `@s` array and reads `$rule`,
//! `$winner`, `%winner`, `$hora`, `$dayname[0]` from `our` slots set
//! by `precedence`. The Rust port takes everything as parameters:
//! `(office: &OfficeOutput, hour: Hour, ordinarium: &[OrdoLine])` and
//! returns a fresh `Vec<RenderedLine>`.

use crate::breviary::horas::Hour;
use crate::core::OfficeOutput;
use crate::data_types::{HorasFile, OrdoLine};
use crate::ordo::RenderedLine;

/// Walker state for one `specials()` invocation. Mirrors the locals
/// the Perl walker maintains (`$skipflag`, `$specialflag`,
/// `$litaniaflag`, `$octavam`, `$tind`, `$item`, `$label`).
#[derive(Debug, Default, Clone)]
pub struct WalkerState {
    /// Skip the next non-section line. Set by section-handlers that
    /// emit their own body and want to swallow the inline-template
    /// body that would otherwise follow.
    pub skip: bool,
    /// "Special conclusion" was emitted; suppress later additions.
    pub special: bool,
    /// Litania (Litany of the Saints) was emitted; suppresses Marian
    /// closing antiphon at Lauds.
    pub litania: bool,
    /// Anti-duplicate guard for octave commemorations.
    pub octavam: String,
    /// Current `#Section` label (for `setbuild` tracing).
    pub current_label: String,
}

/// Drive the `specials` walker over `ordinarium` and produce the
/// rendered output.
///
/// Mirrors `specials.pl::specials` lines 21-408. The loop's outer
/// dispatch table:
///
/// | Section heading match | Helper |
/// |---|---|
/// | `incipit` | guard the `Pater/Ave/Credo` quartet against `Ave only` rule |
/// | `Capitulum Versum 2` (rubric override) | replace Capitulum with versicle |
/// | `Prelude` | splice `Prelude $hora` body if present |
/// | `Commemoratio officii parvi` | splice `COP $hora` from `CommuneM/C12.txt` |
/// | `preces` | dispatch [`crate::breviary::preces`] |
/// | `invitatorium` | dispatch [`crate::breviary::matins::invitatorium`] |
/// | `psalm` | dispatch [`crate::breviary::psalter`] |
/// | `Capitulum` (Prima) | dispatch [`crate::breviary::prima::capitulum_prima`] |
/// | `Lectio brevis` (Compline) | static `Psalterium/Special/Minor Special.txt::Lectio Completorium` |
/// | `Capitulum` (T/S/N/Compl) | dispatch [`crate::breviary::capitulum::capitulum_minor`] |
/// | `Capitulum` (Laudes/Vespera) | dispatch [`crate::breviary::capitulum::capitulum_major`] |
/// | `Responsor` (Monastic Laudes/Vespera) | dispatch [`crate::breviary::capitulum::monastic_major_responsory`] |
/// | `Regula` (Monastic Prima) | dispatch [`crate::breviary::monastic::regula`] |
/// | `Lectio brevis` (Prima) | dispatch [`crate::breviary::prima::lectio_brevis_prima`] |
/// | `Hymnus` | dispatch [`crate::breviary::hymnus::get_hymn`] |
/// | `Canticum` | dispatch [`crate::breviary::canticum::canticum`] |
/// | `Oratio` | dispatch [`crate::breviary::oratio::oratio`] |
/// | `Suffragium` (Laudes/Vespera, pre-1955) | dispatch [`crate::breviary::suffragium::get_suffragium_body`] |
/// | `Martyrologium` (Prima) | dispatch [`crate::breviary::martyrologium::martyrologium`] |
/// | `Commemoratio defunctorum` (Prima) | static `Psalterium/Special/Prima Special.txt` body |
/// | `Antiphona finalis` (Compline) | dispatch [`crate::breviary::canticum::final_marian_antiphon`] |
/// | `Conclusio` | apply Litania-of-Saints insertion if rule says so; apply Special Conclusio if rule says so; apply dirge-of-the-dead substitution at Vespera/Laudes |
///
/// Returns the fully rendered `Vec<RenderedLine>`.
pub fn run_specials_walker(
    _office: &OfficeOutput,
    _hour: Hour,
    _ordinarium: &[OrdoLine],
) -> Vec<RenderedLine> {
    // TODO(B10): port specials.pl:21-408. ~400 LOC of Perl that
    // dispatches to ~25 per-section helpers. Each helper lives in its
    // own module under crate::breviary::*; this fn is the dispatch
    // table.
    unimplemented!("phase B10: specials walker")
}

/// `loadspecial` (`specials.pl:769`). When the winner has a `Special
/// $hora` section, the entire script is replaced with that body. Used
/// for Triduum days where the office is wholly atypical.
pub fn load_special(_special_body: &str) -> Vec<RenderedLine> {
    // TODO(B10): port specials.pl:769-781. Splits the body into lines
    // and emits each as a Plain RenderedLine, with `&macro` and
    // `$prayer` references resolved.
    unimplemented!("phase B10: loadspecial")
}

/// Translate a section label to its localised heading. Mirrors the
/// `translate($label, $lang)` calls scattered through `specials.pl`.
/// Latin-only here; vernacular is downstream.
pub fn translate_label(label: &str) -> String {
    // For Latin-only Office rendering this is the identity function —
    // the section labels are already Latin. Kept as a function so
    // future translation slices can plug in.
    label.to_string()
}

/// Build-script trace insertion. Mirrors `setbuild1` / `setbuild2` /
/// `setbuild` (`specials.pl:658-699`). The Perl walker writes a
/// running `$buildscript` global summarising which sections it
/// included / omitted / substituted; the upstream UI shows it in a
/// "Building Script" panel for debugging.
///
/// In the Rust port this becomes structured events on a tracing
/// channel (or simply discarded under the `regression` feature). The
/// fn signature is here so `specials` calls type-check.
pub fn setbuild(_label: &str, _kind: BuildEventKind, _detail: &str) {
    // TODO(B20): emit structured trace event. For B10 this is a no-op.
}

/// Categories for [`setbuild`] events.
#[derive(Debug, Clone, Copy)]
pub enum BuildEventKind {
    Include,
    Omit,
    Substitute,
    Limit,
    Special,
}

/// Helper used by the walker to fetch the per-day rule body
/// (`$winner{Rule}`). This is `office.rule.iter()...join("\n")`.
pub fn winner_rule(office: &OfficeOutput) -> String {
    office.rule.iter().map(|r| r.0.as_str()).collect::<Vec<_>>().join("\n")
}

/// Detect whether the winner file has a `Special $hora` section.
/// Mirror of `specials.pl:37` `exists($w{"Special $hora$i"})`.
pub fn has_special_for_hour(_winner_file: Option<&HorasFile>, _hour: Hour, _vespera_index: u8) -> bool {
    // TODO(B10): port specials.pl:36-38. Two-keyed lookup —
    //   * Laudes uses suffix `' 2'`
    //   * Vespera uses suffix `' 1'` or `' 3'` (vespera variable)
    //   * other hours use no suffix
    unimplemented!("phase B10: has_special_for_hour")
}
