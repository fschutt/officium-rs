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
