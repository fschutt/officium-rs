//! Build script: transcode the source JSON corpus to postcard.
//!
//! At lib runtime we want to avoid embedding the full JSON parser
//! (`serde_json`) — postcard is much smaller in compiled size and
//! doesn't need `std`. So at *build* time we read each `data/*.json`,
//! deserialize it via `serde_json` into the same struct shapes the
//! lib exposes, and re-encode via `postcard::to_allocvec`. The result
//! lands in `OUT_DIR/<name>.postcard` and is `include_bytes!`'d at
//! runtime.
//!
//! `data_types.rs` is shared between this build script and the lib so
//! the shapes stay in sync — `#[path]` include below pulls in the
//! same source file `crate::data_types` resolves to.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[path = "src/data_types.rs"]
mod data_types;

use data_types::{Cell, HorasFile, KalendariaEntry, MassFile, OrdoCorpus, PsalmFile, SanctiEntry};

fn transcode<T>(input: &Path, out_path: &Path)
where
    T: serde::Serialize + serde::de::DeserializeOwned + std::fmt::Debug,
{
    let bytes = fs::read(input)
        .unwrap_or_else(|e| panic!("read {}: {e}", input.display()));
    let value: T = serde_json::from_slice(&bytes)
        .unwrap_or_else(|e| panic!("parse {}: {e}", input.display()));
    let encoded = postcard::to_allocvec(&value)
        .unwrap_or_else(|e| panic!("postcard {}: {e}", input.display()));
    let compressed = brotli_compress(&encoded);
    fs::write(out_path, &compressed)
        .unwrap_or_else(|e| panic!("write {}: {e}", out_path.display()));
    println!(
        "cargo:warning={}: json {} → postcard {} → brotli {} ({:.1}%)",
        input.file_name().unwrap().to_string_lossy(),
        bytes.len(),
        encoded.len(),
        compressed.len(),
        compressed.len() as f64 / bytes.len() as f64 * 100.0,
    );
}

fn brotli_compress(input: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut out = Vec::with_capacity(input.len() / 3);
    {
        // K2 (slice 1): bump lgwin to 24 (16 MB window). Default
        // 22 = 4 MB; for a 2.5 MB raw `missa_latin.postcard` and
        // 4.3 MB `horas_latin.json` source this is leaving table
        // value on the floor — brotli's match-finder benefits from
        // a larger window when repeated phrases (`Per Dóminum`,
        // `Glória Patri`, etc.) span more than 4 MB of context.
        let params = brotli::enc::BrotliEncoderParams {
            quality: 11,
            lgwin: 24,
            ..Default::default()
        };
        let mut writer = brotli::CompressorWriter::with_params(&mut out, 4096, &params);
        writer
            .write_all(input)
            .expect("brotli write_all should not fail on Vec");
        writer.flush().expect("brotli flush");
    }
    out
}

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR");
    let data = Path::new(&manifest_dir).join("data");
    let out = Path::new(&out_dir);

    // Re-run when any input changes.
    println!("cargo:rerun-if-changed=src/data_types.rs");
    println!("cargo:rerun-if-changed=data/sancti.json");
    println!("cargo:rerun-if-changed=data/kalendaria_1962.json");
    println!("cargo:rerun-if-changed=data/kalendaria_by_rubric.json");
    println!("cargo:rerun-if-changed=data/missa_latin.json");
    println!("cargo:rerun-if-changed=data/ordo_latin.json");
    println!("cargo:rerun-if-changed=data/horas_latin.json");
    println!("cargo:rerun-if-changed=data/psalms_latin.json");

    transcode::<HashMap<String, Vec<SanctiEntry>>>(
        &data.join("sancti.json"),
        &out.join("sancti.postcard.br"),
    );

    transcode::<HashMap<String, Option<KalendariaEntry>>>(
        &data.join("kalendaria_1962.json"),
        &out.join("kalendaria_1962.postcard.br"),
    );

    // kalendaria_by_rubric.json — top-level `{ "1570": { "MM-DD":
    // [Cell, ...] }, "1888": ..., ... }`.
    transcode::<HashMap<String, HashMap<String, Vec<Cell>>>>(
        &data.join("kalendaria_by_rubric.json"),
        &out.join("kalendaria_by_rubric.postcard.br"),
    );

    transcode::<OrdoCorpus>(
        &data.join("ordo_latin.json"),
        &out.join("ordo_latin.postcard.br"),
    );

    // K2 — combined missa+horas brotli stream. The two corpora share
    // most of their liturgical phrasing (every "Per Dóminum", "Glória
    // Patri", "Sicut erat", "℣./℟." marker) and brotli's match-finder
    // benefits from seeing them in one stream: separate compression
    // gives 1,617,856 bytes; concat-and-compress gives 1,350,463
    // bytes (-16.5%). We emit ONE `corpus.postcard.br` containing
    // a small length-prefix header (8 bytes) followed by the two
    // raw postcards back-to-back; the runtime decompresses once and
    // hands each module a slice into the shared blob.
    //
    // Header layout (little-endian u32 × 2):
    //   bytes 0..4   = horas postcard length
    //   bytes 4..8   = missa postcard length
    //   bytes 8..    = horas postcard bytes followed by missa
    let missa_postcard = {
        let bytes = fs::read(data.join("missa_latin.json"))
            .expect("read missa_latin.json");
        let value: HashMap<String, MassFile> = serde_json::from_slice(&bytes)
            .expect("parse missa_latin.json");
        postcard::to_allocvec(&value).expect("postcard missa")
    };
    let horas_postcard: Vec<u8> = {
        let horas_json = data.join("horas_latin.json");
        if horas_json.exists() {
            let bytes = fs::read(&horas_json).expect("read horas_latin.json");
            let value: HashMap<String, HorasFile> = serde_json::from_slice(&bytes)
                .expect("parse horas_latin.json");
            postcard::to_allocvec(&value).expect("postcard horas")
        } else {
            // Empty stub so the runtime always has SOMETHING to read.
            let empty: HashMap<String, HorasFile> = HashMap::new();
            postcard::to_allocvec(&empty).expect("empty horas postcard")
        }
    };
    let mut combined = Vec::with_capacity(8 + horas_postcard.len() + missa_postcard.len());
    combined.extend_from_slice(&(horas_postcard.len() as u32).to_le_bytes());
    combined.extend_from_slice(&(missa_postcard.len() as u32).to_le_bytes());
    combined.extend_from_slice(&horas_postcard);
    combined.extend_from_slice(&missa_postcard);
    let combined_br = brotli_compress(&combined);
    fs::write(out.join("corpus.postcard.br"), &combined_br)
        .expect("write corpus.postcard.br");
    println!(
        "cargo:warning=corpus.postcard.br: horas {} + missa {} → combined {} → brotli {} ({:.1}%)",
        horas_postcard.len(),
        missa_postcard.len(),
        combined.len(),
        combined_br.len(),
        combined_br.len() as f64 / combined.len() as f64 * 100.0,
    );

    let psalms_json = data.join("psalms_latin.json");
    if psalms_json.exists() {
        transcode::<HashMap<String, PsalmFile>>(
            &psalms_json,
            &out.join("psalms_latin.postcard.br"),
        );
    } else {
        let empty: HashMap<String, PsalmFile> = HashMap::new();
        let bytes = postcard::to_allocvec(&empty).expect("empty psalms postcard");
        let compressed = brotli_compress(&bytes);
        std::fs::write(out.join("psalms_latin.postcard.br"), compressed).unwrap();
    }
}
