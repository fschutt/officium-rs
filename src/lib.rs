//! # officium-rs
//!
//! Divinum Officium rubric core in pure Rust. Computes the Roman-rite
//! liturgical calendar and resolves Mass propers for any date under
//! any of five rubric layers — Tridentine 1570 → Rubrics 1960
//! (John XXIII) — with 100% output parity against the upstream Perl
//! implementation across a year-sweep regression (21,900 cells × 5
//! rubrics).
//!
//! ## Status
//!
//! - ✅ Calendar, occurrence, precedence, mass-propers resolution (Latin)
//! - ✅ Mass Ordinary renderer ([`ordo`]) — guard-aware walker over
//!   the upstream `Ordo/Ordo*.txt` template, with side-effect hooks
//!   (Introibo / GloriaM / Credo), hook-guards (CheckBlessing,
//!   CheckUltimaEv, …), and proper-insertion splicing.
//! - ✅ All five rubric layers at 100% Perl parity
//! - ⏳ Monastic rubric
//! - ⏳ Office hours (Vespers, Lauds, …) — only Mass today
//! - ⏳ Translations (English, German, …) — Latin only today
//!
//! ## Architecture
//!
//! The crate exposes a [`corpus::Corpus`] trait + pure functions over
//! it. The default [`corpus::BundledCorpus`] reads from the JSON corpus
//! shipped under `data/` (embedded via `include_str!`); consumers can
//! supply their own impl for custom data sources.

pub mod core;
pub mod corpus;
pub mod data_types;
pub mod date;
pub mod embed;
pub mod kalendaria;
pub mod kalendaria_layers;
pub mod kalendarium_1570;
pub mod mass;
pub mod missa;
pub mod occurrence;
pub mod ordo;
pub mod prayers;
pub mod precedence;
pub mod reform;
pub mod sancti;
pub mod tempora_table;
pub mod transfer_table;
pub mod translation;

#[cfg(all(feature = "regression", not(target_arch = "wasm32")))]
pub mod regression;

#[cfg(all(feature = "regression", target_arch = "wasm32"))]
compile_error!("the `regression` feature is native-only; disable it for wasm32 targets");

#[cfg(feature = "wasm")]
pub mod wasm;
