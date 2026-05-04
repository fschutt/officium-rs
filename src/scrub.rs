//! Render-text scrubs mirroring upstream Perl `webdia.pl::display_text`.
//!
//! `vendor/divinum-officium/web/cgi-bin/horas/webdia.pl:651-682` is
//! the Perl render-layer cleaner — a sequence of `s/.../.../`
//! substitutions applied to each Latin body before HTML emission.
//! This module is the corresponding step in our pipeline: the
//! Mass/Office walkers (`crate::ordo::render_mass`,
//! `crate::horas::compute_office_hour`) call [`scrub_render_text`]
//! on each emitted body so the WASM output matches what Perl would
//! display.
//!
//! Architectural rule: **the scrubs live here, not in the Python
//! extraction layer**. The JSON corpus stays a faithful transcode
//! of the upstream `.txt` files — re-running `data/build_*.py`
//! over a refreshed upstream tree always produces the same shape.
//! All "what the user sees" transforms happen here, where they
//! mirror the Perl behaviour line for line.
//!
//! Adding a new scrub: write a small private fn that takes `&str`
//! and returns `String`, then chain it in [`scrub_render_text`].
//! Each scrub is a generic transform — never hardcode an upstream
//! Latin phrase, since the source text is what gets refreshed
//! periodically from the Perl repo.

/// Apply every render-time scrub in order. Mirror `webdia.pl`.
pub fn scrub_render_text(s: &str) -> String {
    // Cheap fast-path: no scrub triggers anywhere in the body.
    if !needs_scrubbing(s) {
        return s.to_string();
    }
    let s = strip_wait_markers(s);
    // Future scrubs from `webdia.pl` (add here when their absence
    // shows up in render output):
    //   - `s/\_/ /g` — `_` paragraph-separator → space
    //   - `s/\{\:.*?\:\}//sg` — chant-engraving `{:H-foo:}` directives
    //   - `s/\`//g` — editor accent-grave reminder
    s
}

fn needs_scrubbing(s: &str) -> bool {
    // Each scrub adds an early-exit literal here.
    contains_ascii_ci(s, b"wait")
}

/// Strip `wait[0-9]+` directives. Mirrors `webdia.pl:654`:
/// `$text =~ s/wait[0-9]+//ig;`. Replaces each marker with a single
/// space and absorbs adjacent whitespace so we don't introduce
/// double-spaces or stranded leading whitespace.
fn strip_wait_markers(s: &str) -> String {
    let bytes = s.as_bytes();
    let n = bytes.len();
    let mut out = String::with_capacity(n);
    let mut i = 0;
    while i < n {
        if i + 5 <= n
            && bytes[i..i + 4].eq_ignore_ascii_case(b"wait")
            && bytes[i + 4].is_ascii_digit()
        {
            let mut j = i + 5;
            while j < n && bytes[j].is_ascii_digit() {
                j += 1;
            }
            // Drop trailing whitespace already in `out`, emit one
            // space (unless we'd be at the start), then skip any
            // following whitespace in the source.
            while matches!(out.as_bytes().last(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
                out.pop();
            }
            if !out.is_empty() {
                out.push(' ');
            }
            i = j;
            while i < n && matches!(bytes[i], b' ' | b'\t' | b'\n' | b'\r') {
                i += 1;
            }
            continue;
        }
        // Copy one UTF-8 codepoint. `s` is a valid &str so the byte
        // boundaries are guaranteed.
        let cp_len = utf8_codepoint_len(bytes[i]);
        let end = (i + cp_len).min(n);
        if let Ok(piece) = core::str::from_utf8(&bytes[i..end]) {
            out.push_str(piece);
        }
        i = end;
    }
    out
}

fn contains_ascii_ci(s: &str, needle: &[u8]) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() < needle.len() {
        return false;
    }
    for i in 0..=bytes.len() - needle.len() {
        if bytes[i..i + needle.len()].eq_ignore_ascii_case(needle) {
            return true;
        }
    }
    false
}

fn utf8_codepoint_len(b: u8) -> usize {
    match b {
        0..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF7 => 4,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_wait_basic() {
        let s = "Hello wait5 World";
        assert_eq!(scrub_render_text(s), "Hello World");
    }

    #[test]
    fn strip_wait_keeps_inline_punctuation() {
        // The upstream Perl Ordo has `tuarum N. et N. wait10 (Jungit
        // manus, …)`. After scrub, the `(Jungit` should follow with
        // a single space, no leftover `wait10`.
        let s = "tuarum N. et N. wait10 (Jungit manus, orat aliquantulum)";
        let got = scrub_render_text(s);
        assert!(!got.contains("wait"), "wait remained: {got}");
        assert!(
            got.contains("N. et N. (Jungit manus"),
            "join broken: {got}"
        );
        assert!(!got.contains("  "), "double space: {got:?}");
    }

    #[test]
    fn strip_wait_trailing() {
        // Bare `wait5` on its own line (Ordo.txt has these too).
        let s = "Foo\nwait5\nBar";
        let got = scrub_render_text(s);
        assert!(!got.contains("wait"), "wait remained: {got:?}");
    }

    #[test]
    fn strip_wait_case_insensitive() {
        // Perl's `s/wait[0-9]+//ig` is case-insensitive.
        let s = "Foo Wait16 Bar wAiT3 baz";
        let got = scrub_render_text(s);
        assert_eq!(got, "Foo Bar baz");
    }

    #[test]
    fn no_scrubbing_when_no_marker() {
        let s = "Plain text without markers — æíó";
        assert_eq!(scrub_render_text(s), s);
    }

    #[test]
    fn preserves_unicode() {
        let s = "Dómine, ad adjuvándum me festína. wait10 Glória Patri";
        let got = scrub_render_text(s);
        assert!(got.contains("Dómine"));
        assert!(got.contains("Glória Patri"));
        assert!(!got.contains("wait"));
    }

    #[test]
    fn does_not_match_wait_without_digits() {
        // `await`, `waited`, plain `wait` (no digits) — leave untouched.
        let s = "I await the wait period; we waited.";
        let got = scrub_render_text(s);
        assert_eq!(got, s);
    }

    #[test]
    fn empty_and_short_strings() {
        assert_eq!(scrub_render_text(""), "");
        assert_eq!(scrub_render_text("a"), "a");
        assert_eq!(scrub_render_text("wait"), "wait"); // no digits, no match
    }
}
