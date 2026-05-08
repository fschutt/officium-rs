//! Pure-core types for the Divinum Officium port. Inputs, outputs,
//! and the value types they carry. No I/O, no logic — just the
//! boundary.
//!
//! Every Perl `our $foo` global from `horascommon.pl::precedence()`
//! and `missa/propers.pl` ends up as a field on `OfficeOutput` or
//! `MassPropers`. See `DIVINUM_OFFICIUM_PORT_PLAN.md` "Architecture"
//! for the full reasoning.

use std::fmt;

// ─── Date ────────────────────────────────────────────────────────────

/// Gregorian date. Plain value type; `date.rs` carries the math.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Date {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

impl Date {
    pub const fn new(year: i32, month: u32, day: u32) -> Self {
        Self { year, month, day }
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

// ─── Rubric ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Rubric {
    Tridentine1570,
    Tridentine1910,
    DivinoAfflatu1911,
    Reduced1955,
    Rubrics1960,
    Monastic,
}

impl Rubric {
    /// The version-string the upstream Perl `missa.pl` /
    /// `officium.pl` expect via `version=…` on the command line.
    /// Used by the regression harness; do not alter without
    /// confirming against
    /// `vendor/divinum-officium/web/cgi-bin/DivinumOfficium/RunTimeOptions.pm`.
    pub const fn as_perl_version(self) -> &'static str {
        match self {
            Rubric::Tridentine1570    => "Tridentine - 1570",
            // Carry BOTH 1906 and 1910 substrings so the setupstring
            // conditional evaluator's regex-style predicate match
            // accepts BOTH `(rubrica 1906)` (used in Sancti/11-09 to
            // pick "Archibasilicæ;;Duplex majus;;4" under post-1888
            // rubrics) AND `(rubrica 1910)` (used in Holy Week Mass
            // missa/Tempora/Quad6-2 to select the longer Marcus
            // Evangelium reading). Perl's actual version string is
            // "Tridentine - 1906" but its directive set covers
            // 1910 via separate inheritance / token-mapping logic
            // (data.txt's `transferbase` chain) which we approximate
            // by including both substrings in the friendly label.
            Rubric::Tridentine1910    => "Tridentine - 1906/1910",
            Rubric::DivinoAfflatu1911 => "Divino Afflatu",
            Rubric::Reduced1955       => "Reduced - 1955",
            Rubric::Rubrics1960       => "Rubrics 1960 - 1960",
            Rubric::Monastic          => "pre-Trident Monastic",
        }
    }

    /// Filesystem-safe slug.
    pub const fn slug(self) -> &'static str {
        match self {
            Rubric::Tridentine1570    => "trid-1570",
            Rubric::Tridentine1910    => "trid-1910",
            Rubric::DivinoAfflatu1911 => "divino-afflatu",
            Rubric::Reduced1955       => "reduced-1955",
            Rubric::Rubrics1960       => "rubrics-1960",
            Rubric::Monastic          => "monastic",
        }
    }

    pub const ALL_ROMAN: &'static [Rubric] = &[
        Rubric::Tridentine1570,
        Rubric::Tridentine1910,
        Rubric::DivinoAfflatu1911,
        Rubric::Reduced1955,
        Rubric::Rubrics1960,
    ];

    /// The rubric tag used in upstream `Tabulae/Transfer/<letter>.txt`
    /// and `Tabulae/Tempora/Generale.txt` for filtering rubric-
    /// specific entries. Each Rubric has one canonical tag matching
    /// the `transfer` column in upstream's `Tabulae/data.txt`:
    ///   Tridentine - 1570 → 1570
    ///   Tridentine - 1910 → 1906
    ///   Divino Afflatu (1939+1954) → DA
    ///   Reduced - 1955    → 1960  (yes, uses 1960's transfer rules)
    ///   Rubrics 1960      → 1960
    ///   Monastic          → M1617
    pub const fn transfer_rubric_tag(self) -> &'static str {
        match self {
            Rubric::Tridentine1570    => "1570",
            Rubric::Tridentine1910    => "1906",
            Rubric::DivinoAfflatu1911 => "DA",
            Rubric::Reduced1955       => "1960",
            Rubric::Rubrics1960       => "1960",
            Rubric::Monastic          => "M1617",
        }
    }

    /// Map this rubric to its kalendar layer
    /// (`kalendaria_layers::Layer`). Multiple rubrics may share a
    /// layer when the difference between them is rubric-rule changes
    /// rather than kalendar diffs (Tridentine 1570 vs 1910 share the
    /// 1888/1906 kalendar updates as far as the saint table goes).
    ///
    /// Used by `lookup_kalendar_for_rubric` so reform-layer code can
    /// drive the kalendar lookup from the active rubric without each
    /// site re-deciding the mapping.
    pub const fn kalendar_layer(
        self,
    ) -> crate::kalendaria_layers::Layer {
        use crate::kalendaria_layers::Layer;
        match self {
            // Tridentine 1570 baseline.
            Rubric::Tridentine1570    => Layer::Pius1570,
            // Tridentine 1910 = Pius X early kalendar (pre-Divino-Afflatu).
            Rubric::Tridentine1910    => Layer::PiusX1906,
            // Divino Afflatu (1911) — kalendar is the 1939 cumulative
            // (Pius XI added Christ the King 1925, etc.); rubric rules
            // are the 1911 breviary reforms.
            Rubric::DivinoAfflatu1911 => Layer::PiusXI1939,
            Rubric::Reduced1955       => Layer::PiusXII1955,
            Rubric::Rubrics1960       => Layer::JohnXXIII1960,
            // Monastic shares the Tridentine 1570 kalendar baseline
            // (the differences are in the Office cycle, not the saints).
            Rubric::Monastic          => Layer::Pius1570,
        }
    }
}

// ─── Locale ──────────────────────────────────────────────────────────

/// The rubric core operates in Latin; vernacular text is assembled
/// downstream by the translation layer. Held as an enum (not unit)
/// so that adding vernacular routing later is non-breaking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Locale {
    Latin,
}

// ─── OfficeInput ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OfficeInput {
    pub date: Date,
    pub rubric: Rubric,
    pub locale: Locale,
    /// `true` when the caller is rendering the Mass — read the
    /// missa-side file content as-is. Mass-context skips parent
    /// following on `mass_broken_redirect` stubs (only known case:
    /// `Tempora/Pasc1-0t.txt`, where the upstream missa file is
    /// missing the leading `@` so Perl reads it as an empty stub).
    /// Office-context (default) follows the parent chain because
    /// the horas-side file has the proper `@`-prefix and inherits
    /// rank 7 from `Tempora/Pasc1-0`.
    pub is_mass_context: bool,
}

// ─── OfficeOutput ────────────────────────────────────────────────────

/// Everything `precedence()` writes to Perl globals, captured as a
/// single immutable value. Phase 3-4 functions return this; Phase 5
/// `mass_propers()` consumes it.
#[derive(Debug, Clone)]
pub struct OfficeOutput {
    /// The calendar date this office applies to (carried through
    /// from `OfficeInput`). Needed by Mass-side rendering for
    /// date-keyed special cases like the Pope-coronation
    /// anniversary (Commune/Coronatio fires on May 18).
    pub date: Date,
    /// Active rubric (carried through from `OfficeInput`). Drives
    /// layer-aware Mass-side rendering (which `(sed rubrica X)`
    /// conditional applies, era-specific rank/commune lookup, etc.).
    pub rubric: Rubric,
    pub winner: FileKey,
    pub commemoratio: Option<FileKey>,
    pub scriptura: Option<FileKey>,
    pub commune: Option<FileKey>,
    pub commune_type: CommuneType,
    pub rank: Rank,
    pub rule: Vec<RuleLine>,
    pub day_kind: DayKind,
    pub season: Season,
    pub color: Color,
    /// Office-only: first-vespers concurrence with tomorrow. Mass
    /// resolution always sees `None`.
    pub vespers_split: Option<VespersSplit>,
    /// Provenance — which reform layers fired and what each did.
    pub reform_trace: Vec<ReformAction>,
}

// ─── Mass propers ────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct MassPropers {
    pub introitus:    Option<ProperBlock>,
    pub oratio:       Option<ProperBlock>,
    pub lectio:       Option<ProperBlock>,
    pub graduale:     Option<ProperBlock>,
    pub tractus:      Option<ProperBlock>,
    pub sequentia:    Option<ProperBlock>,
    pub evangelium:   Option<ProperBlock>,
    pub offertorium:  Option<ProperBlock>,
    pub secreta:      Option<ProperBlock>,
    pub prefatio:     Option<ProperBlock>,
    pub communio:     Option<ProperBlock>,
    pub postcommunio: Option<ProperBlock>,
    pub commemorations: Vec<MassCommemoration>,
}

#[derive(Debug, Clone)]
pub struct ProperBlock {
    pub latin: String,
    pub source: FileKey,
    /// True when the body was pulled via `@Commune/<key>` fallback
    /// rather than being proper to the winning office.
    pub via_commune: bool,
}

#[derive(Debug, Clone)]
pub struct MassCommemoration {
    pub source: FileKey,
    pub oratio: Option<ProperBlock>,
    pub secreta: Option<ProperBlock>,
    pub postcommunio: Option<ProperBlock>,
}

// ─── Rank ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct Rank {
    pub class: RankClass,
    pub kind: RankKind,
    /// As printed in the rubrics (e.g. "Duplex II classis",
    /// "Semiduplex").
    pub raw_label: String,
    /// Numeric precedence — Perl Sancti convention (1=Simplex,
    /// 2=Semiduplex, 3=Duplex, 5=II classis, 6=I classis, …). Float
    /// because some pre-1960 entries carry .5 increments.
    pub rank_num: f32,
}

/// Coarse precedence class. Lower = higher rank (Class I beats II).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RankClass {
    First = 1,
    Second = 2,
    Third = 3,
    Fourth = 4,
}

impl RankClass {
    pub const fn label(self) -> &'static str {
        match self {
            RankClass::First  => "I classis",
            RankClass::Second => "II classis",
            RankClass::Third  => "III classis",
            RankClass::Fourth => "IV classis",
        }
    }
}

/// Mirrors the Perl `$duplex` enumeration plus a few Phase 4
/// additions (Feria, Commemoration) for cases the Perl globals
/// implicitly handled with absence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RankKind {
    Above,            // 7 — above I classis (Easter Triduum, Christmas Day vigils)
    DuplexIClassis,   // 6
    DuplexIIClassis,  // 5
    DuplexMajus,      // 4
    Duplex,           // 3
    Semiduplex,       // 2
    Simplex,          // 1
    Feria,
    Commemoration,
}

// ─── DayKind / Season / Color / CommuneType ──────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DayKind {
    Sunday,
    Feria,
    Feast,
    OctaveDay,
    Vigil,
    EmberDay,
    RogationDay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Season {
    Advent,
    Christmas,
    Septuagesima,
    Lent,
    Passiontide,
    Easter,
    PentecostOctave,
    PostPentecost,
    PostEpiphany,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Color {
    White,
    Red,
    Green,
    Purple,
    Black,
    Rose,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommuneType {
    None,
    /// `vide C2a-1` — see-also reference; some sections proper, some
    /// drawn from the Common.
    Vide,
    /// `ex C2a-1` — drawn directly from the Common.
    Ex,
}

// ─── FileKey ─────────────────────────────────────────────────────────

/// Typed handle for a Mass / Office data-file key. Mirrors upstream
/// path shape: `Sancti/04-29`, `Tempora/Pasc3-0`, `Commune/C2a-1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileKey {
    pub category: FileCategory,
    pub stem: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FileCategory {
    Sancti,
    Tempora,
    Commune,
    SanctiM,
    SanctiOP,
    SanctiCist,
    Other(String),
}

impl FileKey {
    /// Parse `"Sancti/04-29"` → `FileKey { Sancti, "04-29" }`.
    /// Inputs without a `/` map to `Other("")`.
    pub fn parse(s: &str) -> Self {
        let (cat, stem) = match s.split_once('/') {
            Some(("Sancti", s))     => (FileCategory::Sancti, s),
            Some(("Tempora", s))    => (FileCategory::Tempora, s),
            Some(("Commune", s))    => (FileCategory::Commune, s),
            Some(("SanctiM", s))    => (FileCategory::SanctiM, s),
            Some(("SanctiOP", s))   => (FileCategory::SanctiOP, s),
            Some(("SanctiCist", s)) => (FileCategory::SanctiCist, s),
            Some((other, s))        => (FileCategory::Other(other.to_string()), s),
            None                    => (FileCategory::Other(String::new()), s),
        };
        Self { category: cat, stem: stem.to_string() }
    }

    pub fn render(&self) -> String {
        let prefix = match &self.category {
            FileCategory::Sancti     => "Sancti",
            FileCategory::Tempora    => "Tempora",
            FileCategory::Commune    => "Commune",
            FileCategory::SanctiM    => "SanctiM",
            FileCategory::SanctiOP   => "SanctiOP",
            FileCategory::SanctiCist => "SanctiCist",
            FileCategory::Other(s)   => s.as_str(),
        };
        if prefix.is_empty() {
            self.stem.clone()
        } else {
            format!("{prefix}/{}", self.stem)
        }
    }
}

impl fmt::Display for FileKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.render())
    }
}

// ─── RuleLine ────────────────────────────────────────────────────────

/// A line from the `[Rank]` section's tail. Examples:
/// `"no Gloria"`, `"Credo"`, `"Preface=Communis"`. Phase 1 keeps the
/// raw string; Phases 3–5 parse selectively as the regression harness
/// exposes which switches actually drive output.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RuleLine(pub String);

// ─── ReformAction ────────────────────────────────────────────────────

/// One step in `OfficeOutput.reform_trace`. Records that a layer made
/// a decision — which one, what kind, why. Drives the "compare under
/// each rubric" UI proposed in Phase 12.
#[derive(Debug, Clone)]
pub struct ReformAction {
    pub layer: &'static str,
    pub kind: ReformActionKind,
    pub note: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReformActionKind {
    KalendarSuppressed,
    KalendarDemoted,
    KalendarAdded,
    KalendarTransferred,
    RubricOverride,
    CorpusOverride,
}

// ─── VespersSplit (Office only) ──────────────────────────────────────

/// First-vespers concurrence: today's evening office switches to
/// tomorrow's office at the configured break-point. Always `None`
/// for Mass; populated only when Phase 12+ ships the Diurnal page.
#[derive(Debug, Clone)]
pub struct VespersSplit {
    pub split_at: VespersSplitPoint,
    pub from: FileKey,
    pub to: FileKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VespersSplitPoint {
    AfterCapitulum,
    AtMagnificat,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_format() {
        assert_eq!(Date::new(2026, 4, 30).to_string(), "2026-04-30");
        assert_eq!(Date::new(2026, 12, 25).to_string(), "2026-12-25");
    }

    #[test]
    fn rubric_perl_version_strings() {
        // The exact strings the upstream Perl accepts. Pinned tests —
        // changing these breaks the regression harness wiring.
        assert_eq!(Rubric::Tridentine1570.as_perl_version(),    "Tridentine - 1570");
        assert_eq!(Rubric::Tridentine1910.as_perl_version(),    "Tridentine - 1906/1910");
        assert_eq!(Rubric::DivinoAfflatu1911.as_perl_version(), "Divino Afflatu");
        assert_eq!(Rubric::Reduced1955.as_perl_version(),       "Reduced - 1955");
        assert_eq!(Rubric::Rubrics1960.as_perl_version(),       "Rubrics 1960 - 1960");
        assert_eq!(Rubric::Monastic.as_perl_version(),          "pre-Trident Monastic");
    }

    #[test]
    fn rank_class_ordering() {
        // Class I beats Class II (lower numeric = higher rank).
        assert!(RankClass::First < RankClass::Second);
        assert!(RankClass::Second < RankClass::Third);
        assert!(RankClass::Third < RankClass::Fourth);
    }

    #[test]
    fn file_key_roundtrip() {
        for s in [
            "Sancti/04-29",
            "Tempora/Pasc3-0",
            "Commune/C2a-1",
            "SanctiM/01-01",
            "SanctiOP/04-30",
            "SanctiCist/06-06AV",
            "OtherCategory/foo",
        ] {
            let k = FileKey::parse(s);
            assert_eq!(k.render(), s, "roundtrip mismatch on {s:?}");
            assert_eq!(k.to_string(), s);
        }
    }

    #[test]
    fn file_key_no_slash() {
        let k = FileKey::parse("bare");
        assert_eq!(k.render(), "bare");
        assert!(matches!(k.category, FileCategory::Other(ref s) if s.is_empty()));
    }

    #[test]
    fn office_input_is_copy() {
        // OfficeInput is a hashable input key — needed for memoizing
        // year-sweep results across rubric enumerations.
        let i = OfficeInput {
            date: Date::new(2026, 4, 30),
            rubric: Rubric::Tridentine1570,
            locale: Locale::Latin,
            is_mass_context: true,
        };
        let j = i;       // Copy
        let _set: std::collections::HashSet<OfficeInput> = [i, j].into_iter().collect();
    }

    #[test]
    fn all_roman_excludes_monastic() {
        assert_eq!(Rubric::ALL_ROMAN.len(), 5);
        assert!(!Rubric::ALL_ROMAN.contains(&Rubric::Monastic));
    }

    #[test]
    fn rubric_kalendar_layer_mapping() {
        use crate::kalendaria_layers::Layer;
        assert_eq!(Rubric::Tridentine1570.kalendar_layer(), Layer::Pius1570);
        assert_eq!(Rubric::Tridentine1910.kalendar_layer(), Layer::PiusX1906);
        assert_eq!(Rubric::DivinoAfflatu1911.kalendar_layer(), Layer::PiusXI1939);
        assert_eq!(Rubric::Reduced1955.kalendar_layer(), Layer::PiusXII1955);
        assert_eq!(Rubric::Rubrics1960.kalendar_layer(), Layer::JohnXXIII1960);
        // Monastic shares the 1570 kalendar baseline (rubric-rule
        // differences only).
        assert_eq!(Rubric::Monastic.kalendar_layer(), Layer::Pius1570);
    }
}
