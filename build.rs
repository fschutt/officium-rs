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

use data_types::{Cell, KalendariaEntry, MassFile, SanctiEntry};

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
    fs::write(out_path, &encoded)
        .unwrap_or_else(|e| panic!("write {}: {e}", out_path.display()));
    println!(
        "cargo:warning=transcoded {} → {} bytes ({} → {})",
        input.display(),
        encoded.len(),
        bytes.len(),
        out_path.display(),
    );
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

    transcode::<HashMap<String, Vec<SanctiEntry>>>(
        &data.join("sancti.json"),
        &out.join("sancti.postcard"),
    );

    transcode::<HashMap<String, Option<KalendariaEntry>>>(
        &data.join("kalendaria_1962.json"),
        &out.join("kalendaria_1962.postcard"),
    );

    // kalendaria_by_rubric.json — top-level `{ "1570": { "MM-DD":
    // [Cell, ...] }, "1888": ..., ... }`.
    transcode::<HashMap<String, HashMap<String, Vec<Cell>>>>(
        &data.join("kalendaria_by_rubric.json"),
        &out.join("kalendaria_by_rubric.postcard"),
    );

    transcode::<HashMap<String, MassFile>>(
        &data.join("missa_latin.json"),
        &out.join("missa_latin.postcard"),
    );
}
