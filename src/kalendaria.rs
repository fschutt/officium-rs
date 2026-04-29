//! Resolved 1962 Roman calendar layer on top of the Sancti corpus.
//!
//! The Divinum Officium data ships the calendar as a chain of diffs:
//!
//!   - **Sancti/MM-DD.txt `[Rank]` (default)**  → Divino Afflatu (1954)
//!   - **`Tabulae/Kalendaria/1955.txt`**         → Pius XII reform (1955)
//!   - **`Tabulae/Kalendaria/1960.txt`**         → John XXIII reform (1960)
//!
//! The 1962 typical edition is "1955 with 1960 applied on top". Anything
//! the diffs don't mention keeps its Divino-Afflatu default.
//!
//! `md2json2/data/build_sancti_json.py` merges the two diffs into
//! `kalendaria_1962.json`. This module loads that file and exposes a
//! single `resolve_1962(month, day)` that returns:
//!
//!   * `Resolution::Suppressed` — date marked `XXXXX` in either diff
//!   * `Resolution::Override(...)` — date overridden by 1955 or 1960
//!   * `Resolution::Default(...)` — fall through to the Sancti file
//!   * `Resolution::Ferial` — no Sancti file at all for that fixed date

use crate::divinum_officium::sancti::{self, SanctiEntry};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug, Clone, Deserialize)]
pub struct KalendariaFeast {
    pub name: String,
    pub rank_num: Option<f32>,
    pub sancti_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KalendariaEntry {
    pub main: KalendariaFeast,
    pub commemorations: Vec<KalendariaFeast>,
}

static KALENDARIA_JSON: &str = include_str!("../../data/kalendaria_1962.json");
static PARSED: OnceLock<HashMap<String, Option<KalendariaEntry>>> = OnceLock::new();

fn parsed() -> &'static HashMap<String, Option<KalendariaEntry>> {
    PARSED.get_or_init(|| serde_json::from_str(KALENDARIA_JSON).unwrap_or_default())
}

#[derive(Debug)]
pub enum Resolution<'a> {
    /// Date was marked XXXXX in 1955 or 1960 — no proper feast in
    /// 1962, the day is a feria of the temporal cycle.
    Suppressed,
    /// 1955 or 1960 supplied an override for this date.
    Override(&'a KalendariaEntry),
    /// Neither diff mentions this date; the Divino-Afflatu Sancti
    /// entry is correct for 1962.
    Default(&'a SanctiEntry),
    /// No Sancti file ships for this fixed date — pure feria.
    Ferial,
}

impl<'a> Resolution<'a> {
    /// Convenience: the displayable feast name, if any.
    pub fn name(&self) -> Option<&str> {
        match self {
            Self::Suppressed | Self::Ferial => None,
            Self::Override(e) => Some(&e.main.name),
            Self::Default(e) => Some(&e.name),
        }
    }

    /// Convenience: a one-line rank label fit for the calendar UI.
    pub fn rank_label(&self) -> Option<String> {
        match self {
            Self::Suppressed | Self::Ferial => None,
            Self::Override(e) => e.main.rank_num.map(|r| format!("rank {r}")),
            Self::Default(e) => {
                if e.rank_class.is_empty() {
                    e.rank_num.map(|r| format!("rank {r}"))
                } else {
                    Some(e.rank_class.clone())
                }
            }
        }
    }

    /// Convenience: list of commemoration names. Empty when the
    /// resolution has none (Default / Ferial / Suppressed).
    pub fn commemoration_names(&self) -> Vec<&str> {
        match self {
            Self::Override(e) => e.commemorations.iter().map(|c| c.name.as_str()).collect(),
            _ => Vec::new(),
        }
    }
}

/// Resolve `(month, day)` to the 1962 typical-edition entry. See
/// `Resolution` variants for the four possible outcomes.
pub fn resolve_1962(month: u32, day: u32) -> Resolution<'static> {
    let key = format!("{month:02}-{day:02}");
    if let Some(slot) = parsed().get(&key) {
        match slot {
            None => return Resolution::Suppressed,
            Some(entry) => return Resolution::Override(entry),
        }
    }
    match sancti::lookup_for_1962(month, day) {
        Some(e) => Resolution::Default(e),
        None => Resolution::Ferial,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suppressed_dates() {
        // 05-08 was "S. Apparitio S. Michaelis" in older calendars,
        // suppressed by the 1960 reform.
        assert!(matches!(resolve_1962(5, 8), Resolution::Suppressed));
    }

    #[test]
    fn override_demotes_old_feast() {
        // 05-03 was "Inventio S. Crucis" Duplex II in Divino Afflatu;
        // 1955 demoted to "Ss. Alexandri et sociorum Martyrum" rank 1.
        let r = resolve_1962(5, 3);
        match &r {
            Resolution::Override(e) => {
                assert!(e.main.name.contains("Alexandri"), "got {}", e.main.name);
                assert_eq!(e.main.rank_num, Some(1.0));
            }
            _ => panic!("expected Override, got {r:?}"),
        }
    }

    #[test]
    fn default_falls_through_when_diff_silent() {
        // 04-29 (S. Petri Martyris) isn't in either diff, so Sancti
        // default is the right answer.
        let r = resolve_1962(4, 29);
        match &r {
            Resolution::Default(e) => assert_eq!(e.name, "S. Petri Martyris"),
            _ => panic!("expected Default, got {r:?}"),
        }
    }

    #[test]
    fn ferial_when_no_data() {
        // 02-30 isn't a real date and no Sancti file, no kalendaria entry
        let r = resolve_1962(2, 30);
        assert!(matches!(r, Resolution::Ferial));
    }
}
