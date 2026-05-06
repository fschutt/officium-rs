//! Perl-output cache for the regression harness.
//!
//! The Perl reference render is a pure function of
//! `(date, rubric, hour, divinum-officium-submodule-SHA)` — nothing
//! else affects its output. So we cache HTML to a versioned
//! directory keyed on the upstream SHA and skip the
//! `do_render.sh` invocation entirely on a hit. After a sweep has
//! populated the cache once, every subsequent run against the same
//! upstream SHA is essentially Rust-only — full year × 5 rubrics
//! drops from minutes to ~5 s.
//!
//! Layout:
//!
//!   target/regression-cache/<sha[..12]>/<rubric-slug>/<YYYY>/<MM-DD>.<hour>.html
//!
//! In CI, wrap the cache directory with `actions/cache@v4` keyed on
//! the submodule SHA so the cache survives across workflow runs and
//! only refills when upstream rolls.
//!
//! Disabling: pass `cache_sha = None` to fall through to a fresh
//! Perl render every call (used when the submodule isn't a git
//! tree, e.g. tarball checkout).

#![cfg(feature = "regression")]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// SHA of the vendored Perl reference, computed once per sweep run
/// and reused across all worker threads. `None` if `git rev-parse`
/// fails — caller falls back to no caching.
pub fn perl_submodule_sha(root: &Path) -> Option<String> {
    let out = Command::new("git")
        .args(["-C", "vendor/divinum-officium", "rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?;
    let trimmed = s.trim();
    if trimmed.len() == 40 {
        Some(trimmed.to_string())
    } else {
        None
    }
}

/// Path of the cached Perl HTML for a given coordinate. Short SHA
/// prefix (12 chars) keeps directory names compact while staying
/// unambiguous within the project's history.
pub fn cache_path(
    root: &Path,
    sha: &str,
    rubric_slug: &str,
    year: i32,
    mm: u32,
    dd: u32,
    hour: &str,
) -> PathBuf {
    root.join("target")
        .join("regression-cache")
        .join(&sha[..12])
        .join(rubric_slug)
        .join(format!("{:04}", year))
        .join(format!("{:02}-{:02}.{}.html", mm, dd, hour))
}

/// Cache-aware wrapper. Looks up `(sha, rubric_slug, year, mm, dd,
/// hour)` in `target/regression-cache/`; on hit returns the cached
/// HTML directly, on miss invokes `render_fresh` and writes the
/// result back before returning. `cache_sha = None` disables the
/// cache entirely.
pub fn render_with_cache<F>(
    root: &Path,
    rubric_slug: &str,
    year: i32,
    mm: u32,
    dd: u32,
    hour: &str,
    cache_sha: Option<&str>,
    render_fresh: F,
) -> Result<String, String>
where
    F: FnOnce() -> Result<String, String>,
{
    if let Some(sha) = cache_sha {
        let path = cache_path(root, sha, rubric_slug, year, mm, dd, hour);
        if let Ok(html) = fs::read_to_string(&path) {
            return Ok(html);
        }
        let html = render_fresh()?;
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(&path, &html);
        return Ok(html);
    }
    render_fresh()
}
