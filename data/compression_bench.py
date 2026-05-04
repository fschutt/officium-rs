#!/usr/bin/env python3
"""Compression bench for officium-rs corpus data.

Tries every reasonable encoder against each input and records compressed
size + decode time. Output: a Markdown table on stdout.

Goal: pick the encoding that minimises *served bundle bytes* AND has a
pure-Rust WASM-compatible decoder. We measure encoded bytes (what gets
embedded into the WASM) but report HTTP-layer compressed size too
(what the user actually downloads — GitHub Pages does gzip).

Run with the venv at /tmp/compbench:
    /tmp/compbench/bin/python3 data/compression_bench.py
"""

from __future__ import annotations
import gzip
import json
import os
import sys
import time
import zlib
import io
from pathlib import Path

import brotli
import lz4.frame
import zstandard as zstd


REPO = Path(__file__).resolve().parent.parent
DATA = REPO / "data"
INPUTS = [
    "sancti.json",
    "kalendaria_1962.json",
    "kalendaria_by_rubric.json",
    "missa_latin.json",
]


# ─── Encoders ────────────────────────────────────────────────────────

def enc_raw(b: bytes) -> bytes:
    return b


def enc_minified(b: bytes) -> bytes:
    """Re-emit JSON without whitespace."""
    return json.dumps(json.loads(b), separators=(",", ":")).encode()


def enc_gzip_max(b: bytes) -> bytes:
    return gzip.compress(b, compresslevel=9)


def enc_deflate_max(b: bytes) -> bytes:
    return zlib.compress(b, level=9)


def enc_brotli_max(b: bytes) -> bytes:
    return brotli.compress(b, quality=11)


def enc_brotli_text_max(b: bytes) -> bytes:
    return brotli.compress(b, quality=11, mode=brotli.MODE_TEXT)


def enc_zstd_22(b: bytes) -> bytes:
    cctx = zstd.ZstdCompressor(level=22)
    return cctx.compress(b)


def enc_zstd_19(b: bytes) -> bytes:
    cctx = zstd.ZstdCompressor(level=19)
    return cctx.compress(b)


def enc_lz4_max(b: bytes) -> bytes:
    return lz4.frame.compress(
        b, compression_level=lz4.frame.COMPRESSIONLEVEL_MAX
    )


def enc_zstd_dict(corpus_bytes: bytes, sample_size: int = 32 * 1024):
    """Build a closure that compresses with a zstd dictionary trained
    on the corpus itself."""
    # Train on a sample of the corpus.
    samples = []
    pos = 0
    chunk = 4096
    while pos + chunk <= len(corpus_bytes) and sum(len(s) for s in samples) < sample_size * 8:
        samples.append(corpus_bytes[pos : pos + chunk])
        pos += chunk * 4  # stride
    if not samples:
        return None, b""
    try:
        dictionary = zstd.train_dictionary(sample_size, samples)
    except Exception as e:
        return None, b""
    cctx = zstd.ZstdCompressor(level=22, dict_data=dictionary)

    def _enc(b: bytes) -> bytes:
        return cctx.compress(b)

    return _enc, dictionary.as_bytes()


# ─── Decoders (timing only) ──────────────────────────────────────────

def dec_gzip(b: bytes) -> bytes:
    return gzip.decompress(b)


def dec_deflate(b: bytes) -> bytes:
    return zlib.decompress(b)


def dec_brotli(b: bytes) -> bytes:
    return brotli.decompress(b)


def dec_zstd(b: bytes) -> bytes:
    return zstd.ZstdDecompressor().decompress(b)


def dec_lz4(b: bytes) -> bytes:
    return lz4.frame.decompress(b)


# ─── Bench ───────────────────────────────────────────────────────────

def time_decode(decode, b: bytes, iters: int = 5) -> float:
    """Median wall-clock decode time in milliseconds."""
    times = []
    for _ in range(iters):
        t0 = time.perf_counter()
        decode(b)
        times.append((time.perf_counter() - t0) * 1000.0)
    times.sort()
    return times[len(times) // 2]


def bench_input(path: Path, *, postcard_path: Path | None = None) -> list[dict]:
    raw = path.read_bytes()
    rows = []

    def row(method, encoded, decode_fn, *, gzip_after=True, has_rust_wasm_decoder=True, notes=""):
        size = len(encoded)
        ratio = size / len(raw)
        if decode_fn is None:
            t = 0.0
        else:
            try:
                t = time_decode(decode_fn, encoded)
            except Exception as e:
                t = -1.0
        # Also measure HTTP-layer gzip on top — the actual served bytes.
        served = len(gzip.compress(encoded, compresslevel=9)) if gzip_after else size
        rows.append({
            "method": method,
            "size": size,
            "ratio": ratio,
            "served_gzip": served,
            "decode_ms": t,
            "rust_wasm_ok": has_rust_wasm_decoder,
            "notes": notes,
        })

    # Baselines
    row("raw_json", raw, None, gzip_after=True, has_rust_wasm_decoder=True,
        notes="status quo before postcard")
    row("json_minified", enc_minified(raw), None, gzip_after=True,
        notes="strip whitespace, pre-postcard era")

    # JSON + each compressor
    row("json+gzip(9)", enc_gzip_max(raw), dec_gzip, gzip_after=False,
        notes="miniz_oxide")
    row("json+deflate(9)", enc_deflate_max(raw), dec_deflate, gzip_after=False,
        notes="miniz_oxide raw deflate")
    row("json+brotli(11)", enc_brotli_max(raw), dec_brotli, gzip_after=False,
        notes="brotli-decompressor crate ~50KB")
    row("json+brotli(11,text)", enc_brotli_text_max(raw), dec_brotli, gzip_after=False,
        notes="brotli MODE_TEXT")
    row("json+zstd(22)", enc_zstd_22(raw), dec_zstd, gzip_after=False,
        notes="ruzstd ~70KB pure-rust",)
    row("json+lz4_max", enc_lz4_max(raw), dec_lz4, gzip_after=False,
        notes="lz4_flex ~15KB; fastest decode")

    # Postcard, if available
    if postcard_path and postcard_path.exists():
        pc = postcard_path.read_bytes()
        row("postcard", pc, None, gzip_after=True,
            notes="current shipping format")
        row("postcard+gzip(9)", enc_gzip_max(pc), dec_gzip, gzip_after=False)
        row("postcard+deflate(9)", enc_deflate_max(pc), dec_deflate, gzip_after=False)
        row("postcard+brotli(11)", enc_brotli_max(pc), dec_brotli, gzip_after=False)
        row("postcard+zstd(22)", enc_zstd_22(pc), dec_zstd, gzip_after=False)
        row("postcard+lz4_max", enc_lz4_max(pc), dec_lz4, gzip_after=False)

        # zstd dictionary trained on the postcard bytes themselves
        dict_enc, dict_bytes = enc_zstd_dict(pc)
        if dict_enc is not None and dict_bytes:
            encoded = dict_enc(pc)
            row("postcard+zstd_dict",
                bytes(dict_bytes) + encoded,
                None,  # decoding requires the dict — measured separately
                gzip_after=False,
                notes=f"dict={len(dict_bytes)}B incl. in size")

    return rows


def bench_all() -> list[tuple[str, list[dict]]]:
    out = []
    # We don't ship .postcard files; they're built in OUT_DIR. Re-derive
    # them here for the bench. This requires the postcard Python module —
    # easier path: just call the Rust build script's output if cached.
    # Since OUT_DIR can vary, we encode JSON via a tiny Rust helper or
    # use the existing target dir if a `cargo build --release` ran.
    # Simplest: use serde_json + msgpack-ish manual postcard isn't
    # available in pure Python. Skip the postcard column for files we
    # can't easily transcode here, and pull in the precomputed
    # postcard files from the latest target/release/build/officium-rs-*/out/
    target_out = REPO / "target" / "release"
    candidate_dirs = list(target_out.glob("build/officium-rs-*/out"))
    out_dir = candidate_dirs[0] if candidate_dirs else None

    for name in INPUTS:
        in_path = DATA / name
        pc_path = (out_dir / name.replace(".json", ".postcard")) if out_dir else None
        rows = bench_input(in_path, postcard_path=pc_path)
        out.append((name, rows))
    return out


def fmt_size(n: int) -> str:
    if n < 1024:
        return f"{n}B"
    if n < 1024 * 1024:
        return f"{n / 1024:.1f}KB"
    return f"{n / 1024 / 1024:.2f}MB"


def render_md(results: list[tuple[str, list[dict]]]) -> str:
    lines = []
    lines.append("# Compression bench — officium-rs corpus")
    lines.append("")
    lines.append("Each row: method × file. `size` = embedded bytes. `served` =")
    lines.append("size after HTTP-layer gzip on top (what the user actually")
    lines.append("downloads from GitHub Pages, which auto-gzips). `decode` =")
    lines.append("median Python decode time over 5 iterations (relative")
    lines.append("comparison only — actual WASM decode will differ).")
    lines.append("")
    for name, rows in results:
        # baseline = raw_json
        baseline = next(r for r in rows if r["method"] == "raw_json")["size"]
        lines.append(f"## `{name}` (raw {fmt_size(baseline)})")
        lines.append("")
        lines.append(
            "| method | size | ratio | served (gzip) | decode | rust-wasm | notes |"
        )
        lines.append(
            "|---|---:|---:|---:|---:|:---:|---|"
        )
        for r in rows:
            t = "-" if r["decode_ms"] == 0.0 else f'{r["decode_ms"]:.1f} ms'
            lines.append(
                f"| `{r['method']}` "
                f"| {fmt_size(r['size'])} "
                f"| {r['ratio']*100:5.1f}% "
                f"| {fmt_size(r['served_gzip'])} "
                f"| {t} "
                f"| {'✓' if r['rust_wasm_ok'] else '✗'} "
                f"| {r['notes']} |"
            )
        lines.append("")
    return "\n".join(lines)


if __name__ == "__main__":
    results = bench_all()
    md = render_md(results)
    out = REPO / "docs" / "COMPRESSION_BENCH.md"
    out.write_text(md)
    print(md)
    print(f"\nwritten to {out}")
