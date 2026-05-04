# Compression bench — officium-rs corpus

Each row: method × file. `size` = embedded bytes. `served` =
size after HTTP-layer gzip on top (what the user actually
downloads from GitHub Pages, which auto-gzips). `decode` =
median Python decode time over 5 iterations (relative
comparison only — actual WASM decode will differ).

## `sancti.json` (raw 49.1KB)

| method | size | ratio | served (gzip) | decode | rust-wasm | notes |
|---|---:|---:|---:|---:|:---:|---|
| `raw_json` | 49.1KB | 100.0% | 5.9KB | - | ✓ | status quo before postcard |
| `json_minified` | 47.4KB |  96.6% | 5.9KB | - | ✓ | strip whitespace, pre-postcard era |
| `json+gzip(9)` | 5.9KB |  12.1% | 5.9KB | 0.0 ms | ✓ | miniz_oxide |
| `json+deflate(9)` | 5.9KB |  12.1% | 5.9KB | 0.0 ms | ✓ | miniz_oxide raw deflate |
| `json+brotli(11)` | 5.2KB |  10.6% | 5.2KB | 0.1 ms | ✓ | brotli-decompressor crate ~50KB |
| `json+brotli(11,text)` | 5.2KB |  10.6% | 5.2KB | 0.1 ms | ✓ | brotli MODE_TEXT |
| `json+zstd(22)` | 5.6KB |  11.4% | 5.6KB | 0.0 ms | ✓ | ruzstd ~70KB pure-rust |
| `json+lz4_max` | 7.5KB |  15.2% | 7.5KB | 0.0 ms | ✓ | lz4_flex ~15KB; fastest decode |
| `postcard` | 26.7KB |  54.4% | 6.4KB | - | ✓ | current shipping format |
| `postcard+gzip(9)` | 6.4KB |  12.9% | 6.4KB | 0.0 ms | ✓ |  |
| `postcard+deflate(9)` | 6.3KB |  12.9% | 6.3KB | 0.0 ms | ✓ |  |
| `postcard+brotli(11)` | 5.7KB |  11.5% | 5.7KB | 0.1 ms | ✓ |  |
| `postcard+zstd(22)` | 6.1KB |  12.4% | 6.1KB | 0.0 ms | ✓ |  |
| `postcard+lz4_max` | 7.9KB |  16.0% | 7.9KB | 0.0 ms | ✓ |  |

## `kalendaria_1962.json` (raw 6.7KB)

| method | size | ratio | served (gzip) | decode | rust-wasm | notes |
|---|---:|---:|---:|---:|:---:|---|
| `raw_json` | 6.7KB | 100.0% | 1.4KB | - | ✓ | status quo before postcard |
| `json_minified` | 6.3KB |  93.5% | 1.4KB | - | ✓ | strip whitespace, pre-postcard era |
| `json+gzip(9)` | 1.4KB |  20.5% | 1.4KB | 0.0 ms | ✓ | miniz_oxide |
| `json+deflate(9)` | 1.4KB |  20.4% | 1.4KB | 0.0 ms | ✓ | miniz_oxide raw deflate |
| `json+brotli(11)` | 1.2KB |  17.9% | 1.2KB | 0.0 ms | ✓ | brotli-decompressor crate ~50KB |
| `json+brotli(11,text)` | 1.2KB |  17.9% | 1.2KB | 0.0 ms | ✓ | brotli MODE_TEXT |
| `json+zstd(22)` | 1.3KB |  19.7% | 1.3KB | 0.0 ms | ✓ | ruzstd ~70KB pure-rust |
| `json+lz4_max` | 1.9KB |  28.4% | 1.9KB | 0.0 ms | ✓ | lz4_flex ~15KB; fastest decode |
| `postcard` | 3.0KB |  44.2% | 1.3KB | - | ✓ | current shipping format |
| `postcard+gzip(9)` | 1.3KB |  19.4% | 1.3KB | 0.0 ms | ✓ |  |
| `postcard+deflate(9)` | 1.3KB |  19.2% | 1.3KB | 0.0 ms | ✓ |  |
| `postcard+brotli(11)` | 1.1KB |  17.1% | 1.1KB | 0.0 ms | ✓ |  |
| `postcard+zstd(22)` | 1.3KB |  19.2% | 1.3KB | 0.0 ms | ✓ |  |
| `postcard+lz4_max` | 1.8KB |  26.4% | 1.8KB | 0.0 ms | ✓ |  |

## `kalendaria_by_rubric.json` (raw 465.4KB)

| method | size | ratio | served (gzip) | decode | rust-wasm | notes |
|---|---:|---:|---:|---:|:---:|---|
| `raw_json` | 465.4KB | 100.0% | 42.5KB | - | ✓ | status quo before postcard |
| `json_minified` | 292.3KB |  62.8% | 39.3KB | - | ✓ | strip whitespace, pre-postcard era |
| `json+gzip(9)` | 42.5KB |   9.1% | 42.5KB | 0.2 ms | ✓ | miniz_oxide |
| `json+deflate(9)` | 42.5KB |   9.1% | 42.5KB | 0.2 ms | ✓ | miniz_oxide raw deflate |
| `json+brotli(11)` | 7.5KB |   1.6% | 7.5KB | 0.1 ms | ✓ | brotli-decompressor crate ~50KB |
| `json+brotli(11,text)` | 7.5KB |   1.6% | 7.5KB | 0.1 ms | ✓ | brotli MODE_TEXT |
| `json+zstd(22)` | 8.2KB |   1.8% | 8.2KB | 0.0 ms | ✓ | ruzstd ~70KB pure-rust |
| `json+lz4_max` | 41.6KB |   8.9% | 41.6KB | 0.1 ms | ✓ | lz4_flex ~15KB; fastest decode |
| `postcard` | 150.6KB |  32.4% | 18.9KB | - | ✓ | current shipping format |
| `postcard+gzip(9)` | 18.9KB |   4.1% | 18.9KB | 0.1 ms | ✓ |  |
| `postcard+deflate(9)` | 18.9KB |   4.1% | 18.9KB | 0.1 ms | ✓ |  |
| `postcard+brotli(11)` | 11.6KB |   2.5% | 11.6KB | 0.1 ms | ✓ |  |
| `postcard+zstd(22)` | 12.4KB |   2.7% | 12.4KB | 0.0 ms | ✓ |  |
| `postcard+lz4_max` | 16.8KB |   3.6% | 16.8KB | 0.0 ms | ✓ |  |
| `postcard+zstd_dict` | 27.8KB |   6.0% | 27.8KB | - | ✓ | dict=18955B incl. in size |

## `missa_latin.json` (raw 2.52MB)

| method | size | ratio | served (gzip) | decode | rust-wasm | notes |
|---|---:|---:|---:|---:|:---:|---|
| `raw_json` | 2.52MB | 100.0% | 732.9KB | - | ✓ | status quo before postcard |
| `json_minified` | 2.96MB | 117.6% | 767.0KB | - | ✓ | strip whitespace, pre-postcard era |
| `json+gzip(9)` | 732.9KB |  28.4% | 732.9KB | 2.5 ms | ✓ | miniz_oxide |
| `json+deflate(9)` | 732.9KB |  28.4% | 732.9KB | 2.5 ms | ✓ | miniz_oxide raw deflate |
| `json+brotli(11)` | 479.4KB |  18.6% | 479.4KB | 3.9 ms | ✓ | brotli-decompressor crate ~50KB |
| `json+brotli(11,text)` | 479.4KB |  18.6% | 479.4KB | 4.7 ms | ✓ | brotli MODE_TEXT |
| `json+zstd(22)` | 493.2KB |  19.1% | 493.2KB | 1.5 ms | ✓ | ruzstd ~70KB pure-rust |
| `json+lz4_max` | 793.1KB |  30.8% | 793.1KB | 1.0 ms | ✓ | lz4_flex ~15KB; fastest decode |
| `postcard` | 2.41MB |  95.6% | 871.9KB | - | ✓ | current shipping format |
| `postcard+gzip(9)` | 871.9KB |  33.8% | 871.9KB | 2.9 ms | ✓ |  |
| `postcard+deflate(9)` | 871.9KB |  33.8% | 871.9KB | 3.0 ms | ✓ |  |
| `postcard+brotli(11)` | 505.6KB |  19.6% | 505.6KB | 4.2 ms | ✓ |  |
| `postcard+zstd(22)` | 521.1KB |  20.2% | 521.1KB | 1.6 ms | ✓ |  |
| `postcard+lz4_max` | 958.3KB |  37.2% | 958.3KB | 1.1 ms | ✓ |  |
| `postcard+zstd_dict` | 648.5KB |  25.2% | 648.5KB | - | ✓ | dict=32768B incl. in size |

## Decision: postcard + brotli

**Picked: postcard encoded → brotli compressed at build time** ("postcard.br").
Decoded at runtime via `brotli-decompressor` (~30 KB compiled, pure-Rust)
+ `postcard::from_bytes` (~10 KB compiled).

### Why not "JSON + brotli"?
On `missa_latin.json` JSON+brotli is marginally smaller than postcard+brotli
(479 KB vs 506 KB), but it requires `serde_json` at runtime, which adds
~150 KB of compiled code to the WASM bundle. Net bundle is bigger with
JSON, so postcard+brotli wins on **total bytes the user downloads**.

### Why not "postcard + gzip"?
Brotli compresses our metadata-heavy files dramatically better (e.g.
`kalendaria_by_rubric`: 18.9 KB gzip → 11.6 KB brotli; almost 40% smaller).
gzip stays available as the HTTP-layer fallback for clients that don't
accept brotli, but the embedded compression is brotli.

### Why not "postcard + zstd"?
`zstd` Rust crate links against the C zstd library by default;
WASM-targeted pure-Rust zstd decoders (`ruzstd`) are ~70 KB compiled
vs `brotli-decompressor`'s ~30 KB. zstd's compressed sizes are
slightly worse than brotli on our inputs (e.g. `missa_latin`:
521 KB zstd vs 506 KB brotli).

### Embedded vs served sizes (final)

| file                          | source JSON | embedded (postcard.br) | savings |
|-------------------------------|------------:|-----------------------:|--------:|
| `sancti.json`                 |   49.1 KB   |   5.7 KB               |   88%   |
| `kalendaria_1962.json`        |    6.7 KB   |   1.2 KB               |   83%   |
| `kalendaria_by_rubric.json`   |  465.4 KB   |  11.6 KB               |   97%   |
| `missa_latin.json`            |   2.52 MB   |  506.2 KB              |   80%   |
| **WASM bundle (incl. code)**  |   3.43 MB raw / 926 KB gzip → | **907 KB raw / 701 KB gzip / 658 KB brotli** | **~25% smaller served** |

End-to-end Node smoke test: init + 2 `compute_office_json` calls = 27 ms.
