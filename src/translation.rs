//! Latin tokenization for client-side translation overlay.
//!
//! Splits a Latin text body into tokens (words, punctuation, macros)
//! and emits HTML where each word is wrapped in a `<span class="lat-tok">`
//! element. The browser-side translation overlay (see the inline JS in
//! `missal.rs`) walks these spans, looks each surface form up in a
//! shipped dictionary, and inserts a `<span class="gloss">` underneath
//! the matched word(s).
//!
//! Per `DIVINUM_OFFICIUM_PLAN.md` this is **Phase 1** of the four-layer
//! pipeline: surface-form lookup against a hand-curated dictionary,
//! suitable for the most common liturgical phrases. Phase 2 (Whitaker's
//! morphology + lemma lookup) will replace `data-lat="<surface>"` with
//! `data-lemma="<lemma>" data-morph="<tag>"` produced at SSG build time.

/// Wrap each Latin word in a span; pass macro lines (those leading with
/// `!`, `&`, `$`) through unchanged so callers can still style them as
/// directive markers.
pub fn tokenize_latin_html(input: &str) -> String {
    let mut out = String::with_capacity(input.len() * 2);
    for line in input.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('!') || trimmed.starts_with('&') || trimmed.starts_with('$') {
            // Caller has chosen to render macros separately; we leave
            // them alone — they aren't liturgical text.
            out.push_str("<span class=\"macro\">");
            out.push_str(&html_escape(line));
            out.push_str("</span>\n");
            continue;
        }
        emit_tokenized_line(line, &mut out);
        out.push('\n');
    }
    out
}

fn emit_tokenized_line(line: &str, out: &mut String) {
    let mut word = String::new();
    for ch in line.chars() {
        if is_latin_letter(ch) {
            word.push(ch);
        } else {
            if !word.is_empty() {
                out.push_str("<span class=\"lat-tok\" data-lat=\"");
                push_attr(out, &fold_for_lookup(&word));
                out.push_str("\">");
                out.push_str(&html_escape(&word));
                out.push_str("</span>");
                word.clear();
            }
            push_char_escaped(out, ch);
        }
    }
    if !word.is_empty() {
        out.push_str("<span class=\"lat-tok\" data-lat=\"");
        push_attr(out, &fold_for_lookup(&word));
        out.push_str("\">");
        out.push_str(&html_escape(&word));
        out.push_str("</span>");
    }
}

/// A "letter" for liturgical-Latin purposes. `is_alphabetic` already
/// covers the diacritics we care about (á é í ó ú ý æ œ ǽ etc.) and
/// rejects punctuation, but it also accepts non-Latin scripts; that's
/// fine for this corpus since stray non-Latin letters (e.g. Hebrew
/// `Adonai`-style insertions) are still semantic words.
fn is_latin_letter(ch: char) -> bool {
    ch.is_alphabetic() || ch == '\u{0300}' || ch == '\u{0301}'
}

/// Normalize a Latin surface form for dictionary lookup. The shipped
/// dictionary is keyed by the diacritic-stripped, lower-case form so
/// the JS layer doesn't have to carry a Unicode normalizer.
fn fold_for_lookup(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        let folded = match ch {
            'á' | 'à' | 'â' | 'ä' | 'ǎ' | 'Á' | 'À' | 'Â' | 'Ä' | 'Ǎ' => 'a',
            'é' | 'è' | 'ê' | 'ë' | 'ě' | 'É' | 'È' | 'Ê' | 'Ë' | 'Ě' => 'e',
            'í' | 'ì' | 'î' | 'ï' | 'ǐ' | 'Í' | 'Ì' | 'Î' | 'Ï' | 'Ǐ' => 'i',
            'ó' | 'ò' | 'ô' | 'ö' | 'ǒ' | 'Ó' | 'Ò' | 'Ô' | 'Ö' | 'Ǒ' => 'o',
            'ú' | 'ù' | 'û' | 'ü' | 'ǔ' | 'Ú' | 'Ù' | 'Û' | 'Ü' | 'Ǔ' => 'u',
            'ý' | 'ÿ' | 'Ý' | 'Ÿ' => 'y',
            'æ' | 'Æ' => 'a', // shipped dict uses "ae" → folded to "a" + "e"
            'œ' | 'Œ' => 'o',
            'ǽ' | 'Ǽ' => 'a',
            other => {
                for lc in other.to_lowercase() {
                    out.push(lc);
                }
                continue;
            }
        };
        if matches!(ch, 'æ' | 'Æ' | 'ǽ' | 'Ǽ') {
            out.push(folded);
            out.push('e');
        } else if matches!(ch, 'œ' | 'Œ') {
            out.push(folded);
            out.push('e');
        } else {
            out.push(folded);
        }
    }
    out
}

fn push_attr(out: &mut String, s: &str) {
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("&quot;"),
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
}

fn push_char_escaped(out: &mut String, ch: char) {
    match ch {
        '&' => out.push_str("&amp;"),
        '<' => out.push_str("&lt;"),
        '>' => out.push_str("&gt;"),
        _ => out.push(ch),
    }
}

fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        push_char_escaped(&mut out, ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_a_simple_phrase() {
        let h = tokenize_latin_html("Pater noster, qui es in cælis.");
        assert!(h.contains(r#"<span class="lat-tok" data-lat="pater">Pater</span>"#));
        assert!(h.contains(r#"<span class="lat-tok" data-lat="noster">noster</span>"#));
        // Comma + space passed through.
        assert!(h.contains(r#"</span>, <span"#));
        // Æ folded to "ae" in the lookup attr.
        let h2 = tokenize_latin_html("cælis sǽculorum");
        assert!(h2.contains("data-lat=\"caelis\""));
        assert!(h2.contains("data-lat=\"saeculorum\""));
    }

    #[test]
    fn passes_macro_lines_through() {
        let h = tokenize_latin_html("!Ps 65:1-2.\nv. Jubiláte Deo.");
        assert!(h.contains(r#"<span class="macro">!Ps 65:1-2.</span>"#));
        assert!(h.contains(r#"<span class="lat-tok" data-lat="jubilate">"#));
    }

    #[test]
    fn folds_diacritics() {
        assert_eq!(fold_for_lookup("Pétri"), "petri");
        assert_eq!(fold_for_lookup("Mártyris"), "martyris");
        assert_eq!(fold_for_lookup("cæli"), "caeli");
    }
}
