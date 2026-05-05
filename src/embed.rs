//! Embedded-data decompression helpers.
//!
//! `build.rs` produces `OUT_DIR/<name>.postcard.br` files (postcard
//! encoded, then brotli-compressed). At runtime we `include_bytes!`
//! them, brotli-decompress on first access, and postcard-decode the
//! resulting bytes.
//!
//! Brotli + postcard was picked after benchmarking eight encoders
//! (see `docs/COMPRESSION_BENCH.md`). Headline: brotli compresses
//! the postcard output ~4-15× depending on input redundancy; the
//! decompressor crate (`brotli-decompressor`) is small (~30-50 KB
//! compiled) and pure-Rust so it fits the WASM target without a C
//! dependency.

use std::io::Read;
use std::sync::OnceLock;

/// Brotli-decompress an embedded `.postcard.br` blob into a fresh
/// `Vec<u8>`. Panics on malformed input — these are build-time
/// artefacts, so failures indicate a build/runtime version skew not
/// a recoverable error.
pub fn decompress(compressed: &[u8]) -> Vec<u8> {
    let mut decoder = brotli_decompressor::Decompressor::new(compressed, 4096);
    let mut out = Vec::with_capacity(compressed.len() * 4);
    decoder
        .read_to_end(&mut out)
        .expect("brotli decompress: malformed embedded data");
    out
}

// ─── Combined corpus blob (K2) ───────────────────────────────────────
//
// `build.rs` packs `horas_latin.postcard` + `missa_latin.postcard`
// into one brotli stream. Sharing the compression context across
// both corpora (which share most of their liturgical phrasing) cuts
// 16% off the brotli output vs separate compression: ~1.62 MB →
// ~1.35 MB.
//
// Header (8 bytes, little-endian u32 × 2):
//   0..4 = horas postcard length
//   4..8 = missa postcard length
//   8..  = horas bytes followed by missa bytes
//
// We decompress the whole thing once into a `Vec<u8>` stored in a
// `OnceLock`, then expose `&'static [u8]` slices for the two
// portions. Each consumer postcard-decodes its own slice on first
// access (still gated by its own `OnceLock`).

static CORPUS_BR: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/corpus.postcard.br"));

static CORPUS_BLOB: OnceLock<Vec<u8>> = OnceLock::new();

fn corpus_blob() -> &'static [u8] {
    CORPUS_BLOB
        .get_or_init(|| decompress(CORPUS_BR))
        .as_slice()
}

/// Slice covering the horas postcard inside the combined blob.
pub fn horas_postcard() -> &'static [u8] {
    let blob = corpus_blob();
    let horas_len = u32::from_le_bytes([blob[0], blob[1], blob[2], blob[3]]) as usize;
    &blob[8..8 + horas_len]
}

/// Slice covering the missa postcard inside the combined blob.
pub fn missa_postcard() -> &'static [u8] {
    let blob = corpus_blob();
    let horas_len = u32::from_le_bytes([blob[0], blob[1], blob[2], blob[3]]) as usize;
    let missa_len = u32::from_le_bytes([blob[4], blob[5], blob[6], blob[7]]) as usize;
    let start = 8 + horas_len;
    &blob[start..start + missa_len]
}
