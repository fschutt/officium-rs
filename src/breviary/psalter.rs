//! Psalter — weekly distribution of psalms across the hours.
//!
//! Mirror of `vendor/divinum-officium/web/cgi-bin/horas/specials/psalmi.pl`:
//!
//! - `psalmi($lang)` (line 5) — the dispatcher; routes to
//!   `psalmi_minor` or `psalmi_major` based on the active hour.
//! - `psalmi_minor($lang)` (line 29) — Tertia / Sexta / Nona /
//!   Compline; pulls from `Psalterium/Psalmi/Psalmi minor.txt`
//!   `[Tertia]` / `[Sexta]` / `[Nona]` / `[Completorium]` blocks.
//! - `psalmi_major($lang)` (line 323) — Lauds / Vespera; pulls from
//!   `Psalterium/Psalmi/Psalmi major.txt` `[Day0 Laudes1]` /
//!   `[Day0 Laudes2]` / `[Day0 Vespera]` … `[Day6 Vespera]`
//!   blocks, plus Paschal-tide and Monastic overrides.
//! - `antetpsalm` (line 637) — formats one antiphon + psalm group;
//!   see [`crate::breviary::antetpsalm`].
//! - `get_stThomas_feria` (line 685) — Saturday-of-Advent-3 St Thomas
//!   the Apostle pre-emption (the Sancti calendar slot displaces the
//!   Tempora ferial psalmody on that day).
//!
//! ## Cursus / version selection
//!
//! The psalter index file is layered:
//!
//! - **Roman post-DA** — `[Day0]`–`[Day6]` blocks (default for Divino /
//!   1955 / 1960).
//! - **Tridentine 1570 / 1910** — uses `Tridentinum=…` keyed values
//!   inside each block (older Sunday Lauds-schema-1 with Ps 117, etc.).
//! - **Pius-X 1911 schema** — same blocks as post-DA but Lauds / Prime
//!   psalms are shuffled.
//! - **Monastic** — uses `Monastic=…` keyed values; out of scope for
//!   first parity pass.
//! - **Cistercian / Praedicatorum** — separate top-level keys; out of
//!   scope.
//!
//! Each rubric layer reads a different combination of these.

use crate::breviary::horas::Hour;
use crate::core::{Date, OfficeOutput};
use crate::ordo::RenderedLine;

/// One psalm in a rendered hour: antiphon + psalm body, with optional
/// Gloria Patri suffix and per-rubric "in directum" flag (no antiphon).
///
/// Postcard-friendly so the regression harness can serialize cells
/// for diff against the Perl oracle.
#[derive(Debug, Clone)]
pub struct PsalmRendered {
    /// Psalm number — `19`, `50`, `94c`, `109`. Allowed letter
    /// suffixes: `a`/`b`/`c` for split psalms.
    pub number: String,
    /// Antiphon body (Latin). Empty when sung "in directum".
    pub antiphon: String,
    /// Optional pre-resolved psalm body. When `None`, the renderer
    /// looks up the body via [`crate::breviary::corpus::psalm`].
    pub body: Option<String>,
    /// Whether to append "Glória Patri" / Doxology at the end.
    /// Suppressed during the Triduum and on Office of the Dead.
    pub gloria_patri: bool,
    /// Whether the antiphon is "duplex" (sung in full before AND
    /// after the psalm) — vs. "intoned" (sung in part before, in full
    /// after). Drives a render-time formatting toggle.
    pub duplex: bool,
}

/// Dispatch — pick `psalmi_minor` or `psalmi_major` based on the hour.
/// Mirror of `psalmi.pl::psalmi` (line 5).
pub fn psalmi(_office: &OfficeOutput, _hour: Hour) -> Vec<RenderedLine> {
    // TODO(B15): port specials/psalmi.pl:5-28. Trivial dispatch:
    //   * Lauds | Vespera         → psalmi_major
    //   * Prima | Tertia | Sexta | Nona | Completorium → psalmi_minor
    //   * Matutinum               → see crate::breviary::matins::psalmody
    unimplemented!("phase B15: psalmi dispatcher")
}

/// Small-hours psalter — Tertia / Sexta / Nona / Compline / Prime.
///
/// Mirror of `psalmi_minor($lang)` (line 29). Reads
/// `Psalterium/Psalmi/Psalmi minor.txt`:
///
/// - `[Prima]` block has `Dominica` / `Feria II` … `Sabbato` keyed sub-blocks
/// - `[Tertia]`/`[Sexta]`/`[Nona]` are bare lists of three psalms each
/// - `[Completorium]` is a fixed list (mostly Ps 4 / 90 / 133)
/// - Tempora antiphon overrides live in `[Adv1]`/`[Quad1]`/`[Pasc]` etc.
///   (same file, lower in the body)
pub fn psalmi_minor(_office: &OfficeOutput, _hour: Hour) -> Vec<PsalmRendered> {
    // TODO(B15): port specials/psalmi.pl:29-322 (~290 LOC). The dense
    // bits are the Pius-X / Tridentine variant selection (4 paths)
    // and the per-tempora antiphon override (5 seasons).
    unimplemented!("phase B15: psalmi_minor")
}

/// Lauds + Vespers psalter.
///
/// Mirror of `psalmi_major($lang)` (line 323). Reads
/// `Psalterium/Psalmi/Psalmi major.txt`. Layered keys:
///
/// - `[Day{0..6} Laudes1]` — traditional Lauds schema (Ps 92 on Sunday)
/// - `[Day{0..6} Laudes2]` — penitential / festal Lauds schema (Ps 50)
/// - `[Day{0..6} Vespera]` — Vespers psalmody
/// - `[Daya{0,C,P,1..6} Laudes]` — Paschal-tide variants (Sunday a vs C
///   indicates a vs c-paschal; explicit feast variants for Sunday)
/// - `[Monastic Laudes]` / `[Monastic Vespera]` — Monastic cursus
/// - `[Cistercian Laudes]` — Cistercian variant
///
/// The selector reads `office.season`, `office.day_kind`, `office.rubric`,
/// `office.rule` and the day-of-week (computed from `office.date`).
pub fn psalmi_major(_office: &OfficeOutput, _hour: Hour) -> Vec<PsalmRendered> {
    // TODO(B15): port specials/psalmi.pl:323-636 (~310 LOC). The
    // densest single chunk in this leg.
    //
    // Decision branches:
    //   * Festal vs ferial Lauds (drives Lauds1 vs Lauds2)
    //   * Paschal flag — `[Daya{0,C,P,1..6} Laudes]`
    //   * Sunday-Lauds-1 vs Sunday-Lauds-2 (canticum vs Daniel)
    //   * Monastic / Cistercian short-circuit (out of scope first pass)
    //
    // Each branch produces a Vec<PsalmRendered>; the caller wraps in
    // `RenderedLine`s.
    unimplemented!("phase B15: psalmi_major")
}

/// Saturday-of-Advent-3 St Thomas-the-Apostle pre-emption. When
/// 21 December (St Thomas) falls on a Saturday in Advent, the
/// Tempora ferial psalmody is replaced with the apostle's psalmody.
///
/// Mirror of `get_stThomas_feria` (line 685).
pub fn get_st_thomas_feria(_date: Date) -> bool {
    // TODO(B15): port specials/psalmi.pl:685-692. Trivial date check.
    unimplemented!("phase B15: get_st_thomas_feria")
}
