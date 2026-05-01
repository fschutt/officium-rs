//! 1570 (Pius V) kalendar override.
//!
//! Loads `vendor/divinum-officium/web/www/Tabulae/Kalendaria/1570.txt`
//! (vendored as `data/kalendarium_1570.txt`) and exposes a per-date
//! lookup that returns the Tridentine winner Sancti-stem plus any
//! commemorations.
//!
//! Format (one entry per fixed date):
//!
//!     *January*
//!     MM-DD=MAIN_STEM[~COMMEMORATION_STEM]=NAME=RANK=[NAME2=RANK2=…]
//!
//! Examples:
//!
//!     01-23=01-23o=S. Emerentianae Virginis et Martyris=1=
//!         → 01-23 winner is `Sancti/01-23o` (the `o` variant), rank 1
//!           (Simplex). Replaces the post-1570 `Sancti/01-23` (Raymond,
//!           instituted 1601).
//!
//!     01-11=01-11~01-11cc=Sexta die infra Octavam Epiphaniae=2=S. Hygini Papæ et Martyris=1=
//!         → 01-11 winner is `Sancti/01-11` (rank 2, Semiduplex), with
//!           commemoration `Sancti/01-11cc` (S. Hyginus, rank 1).
//!
//!     01-12=01-12t=Dominica infra Octavam Epiphaniae=2=
//!         → 01-12 winner is `Sancti/01-12t` (the `t`-suffixed
//!           Tridentine variant — actually a transferred Sunday).
//!
//! The numeric rank column uses Perl Sancti convention:
//! 1 = Simplex, 1.1 = `;;Simplex;;1.1`, 2 = Semiduplex, 3 = Duplex,
//! 4 = Duplex Majus, 5 = II classis, 6 = I classis, 7+ = privileged.

use std::collections::BTreeMap;
use std::sync::OnceLock;

static KAL_TXT: &str = include_str!("../../data/kalendarium_1570.txt");
static PARSED: OnceLock<BTreeMap<(u32, u32), Entry1570>> = OnceLock::new();

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

fn parsed() -> &'static BTreeMap<(u32, u32), Entry1570> {
    PARSED.get_or_init(|| parse(KAL_TXT))
}

/// Look up the 1570 entry for `(month, day)`. Returns `None` when the
/// kalendar table doesn't list this date — the consumer should fall
/// back to the temporal cycle (a feria).
pub fn lookup(month: u32, day: u32) -> Option<&'static Entry1570> {
    parsed().get(&(month, day))
}

fn parse(text: &str) -> BTreeMap<(u32, u32), Entry1570> {
    let mut out: BTreeMap<(u32, u32), Entry1570> = BTreeMap::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('*') || line.starts_with('#') {
            continue;
        }
        if let Some(entry) = parse_line(line) {
            let (md, e) = entry;
            out.insert(md, e);
        }
    }
    out
}

fn parse_line(line: &str) -> Option<((u32, u32), Entry1570)> {
    // Split on `=` into pieces. Format:
    //   MM-DD = MAIN_STEM[~COMMEMORATION_STEM] = NAME = RANK = [NAME2 = RANK2 =]
    // The trailing `=` after the last rank is conventional; we tolerate
    // both presence and absence.
    let parts: Vec<&str> = line.split('=').collect();
    if parts.len() < 4 {
        return None;
    }
    let date_str = parts[0].trim();
    let (mm, dd) = parse_date(date_str)?;

    let stems = parts[1].trim();
    let (main_stem, commemo_stems) = parse_stems(stems);

    // Walk pairs of (name, rank) starting at index 2. There may be
    // 1 main name+rank plus N commemoration name+rank pairs.
    let mut feasts: Vec<(String, f32)> = Vec::new();
    let mut i = 2;
    while i + 1 < parts.len() {
        let name = parts[i].trim();
        let rank_str = parts[i + 1].trim();
        if name.is_empty() && rank_str.is_empty() {
            i += 2;
            continue;
        }
        let rank: f32 = rank_str.parse().unwrap_or(0.0);
        feasts.push((name.to_string(), rank));
        i += 2;
    }
    if feasts.is_empty() {
        return None;
    }

    let main_feast = Feast1570 {
        stem: main_stem,
        name: feasts[0].0.clone(),
        rank_num: feasts[0].1,
    };
    let commemorations: Vec<Feast1570> = commemo_stems
        .into_iter()
        .zip(feasts.iter().skip(1).cloned())
        .map(|(stem, (name, rank))| Feast1570 {
            stem,
            name,
            rank_num: rank,
        })
        .collect();

    Some(((mm, dd), Entry1570 {
        main: main_feast,
        commemorations,
    }))
}

fn parse_date(s: &str) -> Option<(u32, u32)> {
    let (mm_s, dd_s) = s.split_once('-')?;
    let mm: u32 = mm_s.parse().ok()?;
    let dd: u32 = dd_s.parse().ok()?;
    if !(1..=12).contains(&mm) || !(1..=31).contains(&dd) {
        return None;
    }
    Some((mm, dd))
}

fn parse_stems(s: &str) -> (String, Vec<String>) {
    // Split on `~` — first part is the main stem, remainder are
    // commemoration stems. Whitespace-trim each.
    let mut parts = s.split('~').map(str::trim);
    let main = parts.next().unwrap_or("").to_string();
    let commemos: Vec<String> = parts.filter(|p| !p.is_empty()).map(String::from).collect();
    (main, commemos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_emerentiana_replaces_raymond() {
        // 01-23 in 1570 is Emerentiana (Simplex), file stem `01-23o`.
        // Modern Sancti/01-23 is Raymond (1601 institution).
        let e = lookup(1, 23).expect("01-23 entry");
        assert_eq!(e.main.stem, "01-23o");
        assert!(e.main.name.contains("Emerentian"));
        assert_eq!(e.main.rank_num, 1.0);
        assert!(e.commemorations.is_empty());
    }

    #[test]
    fn parse_octave_of_epiphany_with_commemoration() {
        // 01-11 = main feast Sancti/01-11 (Sexta die — Semiduplex,
        // rank 2) + commemoration Sancti/01-11cc (S. Hyginus,
        // Simplex, rank 1).
        let e = lookup(1, 11).expect("01-11 entry");
        assert_eq!(e.main.stem, "01-11");
        assert_eq!(e.main.rank_num, 2.0);
        assert!(e.main.name.contains("Octavam"));
        assert_eq!(e.commemorations.len(), 1);
        assert_eq!(e.commemorations[0].stem, "01-11cc");
        assert_eq!(e.commemorations[0].rank_num, 1.0);
    }

    #[test]
    fn parse_dominica_infra_octavam_uses_t_variant() {
        let e = lookup(1, 12).expect("01-12 entry");
        assert_eq!(e.main.stem, "01-12t");
    }

    #[test]
    fn parse_epiphany_high_rank() {
        let e = lookup(1, 6).expect("01-06 entry");
        assert!(e.main.name.contains("Epiphani"));
        assert_eq!(e.main.rank_num, 6.0);
    }

    #[test]
    fn parse_in_octava_stephani() {
        let e = lookup(1, 2).expect("01-02 entry");
        assert!(e.main.name.contains("Octava"), "{}", e.main.name);
        assert_eq!(e.main.rank_num, 3.0);
    }

    #[test]
    fn missing_date_returns_none() {
        // 02-30 doesn't exist in any kalendar.
        assert!(lookup(2, 30).is_none());
    }

    #[test]
    fn parse_line_synthetic() {
        let line = "01-23=01-23o=S. Emerentianae Virginis et Martyris=1=";
        let ((mm, dd), e) = parse_line(line).expect("parse");
        assert_eq!((mm, dd), (1, 23));
        assert_eq!(e.main.stem, "01-23o");
        assert_eq!(e.main.rank_num, 1.0);
    }

    #[test]
    fn parse_line_with_commemoration() {
        let line = "01-11=01-11~01-11cc=Sexta die infra Octavam Epiphaniae=2=S. Hygini Papæ et Martyris=1=";
        let ((mm, dd), e) = parse_line(line).expect("parse");
        assert_eq!((mm, dd), (1, 11));
        assert_eq!(e.main.stem, "01-11");
        assert_eq!(e.main.rank_num, 2.0);
        assert_eq!(e.commemorations.len(), 1);
        assert_eq!(e.commemorations[0].stem, "01-11cc");
        assert_eq!(e.commemorations[0].rank_num, 1.0);
    }
}
