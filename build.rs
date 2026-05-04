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
        let params = brotli::enc::BrotliEncoderParams {
            quality: 11,
            lgwin: 22,
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

    transcode::<HashMap<String, MassFile>>(
        &data.join("missa_latin.json"),
        &out.join("missa_latin.postcard.br"),
    );

    transcode::<OrdoCorpus>(
        &data.join("ordo_latin.json"),
        &out.join("ordo_latin.postcard.br"),
    );

    // Breviary corpus — only encoded if the JSON file exists. The
    // upstream tree is large (~4.5 MB raw → ~700 KB brotli) so this
    // is gated by the existence of the JSON; runs of `data/build_
    // horas_json.py` produce it. Lib code that depends on this
    // (`src/horas.rs`) must handle the corpus being empty / missing
    // until B1+ ships completely.
    let horas_json = data.join("horas_latin.json");
    if horas_json.exists() {
        transcode::<HashMap<String, HorasFile>>(
            &horas_json,
            &out.join("horas_latin.postcard.br"),
        );
    } else {
        // Write an empty postcard blob so include_bytes! has
        // something to pull at runtime.
        let empty: HashMap<String, HorasFile> = HashMap::new();
        let bytes = postcard::to_allocvec(&empty).expect("empty horas postcard");
        let compressed = brotli_compress(&bytes);
        std::fs::write(out.join("horas_latin.postcard.br"), compressed).unwrap();
    }

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
