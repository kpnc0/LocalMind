use std::path::Path;
use crate::extract::extract_text;

pub fn extract_snippet(path: &Path, query: &str, use_color: bool) -> Option<String> {
    let content = extract_text(path).ok()?;
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return None;
    }

    let query_terms: Vec<String> = query
        .split_whitespace()
        .map(|s| s.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase())
        .filter(|s| s.len() >= 2)
        .collect();

    // Score 3-line sliding windows
    let window_size = 3;
    let mut best_start = 0;
    let mut best_score = 0;

    for i in 0..lines.len() {
        let end = (i + window_size).min(lines.len());
        let mut score = 0;
        let mut terms_hit = 0;

        for line in &lines[i..end] {
            let lower = line.to_lowercase();
            for term in &query_terms {
                if lower.contains(term) {
                    score += 10;
                    terms_hit += 1;
                }
            }
        }
        score += terms_hit * 5;

        if score > best_score {
            best_score = score;
            best_start = i;
        }
    }

    // If no terms matched, find first window with non-empty text
    if best_score == 0 {
        for (i, line) in lines.iter().enumerate() {
            if !line.trim().is_empty() {
                best_start = i;
                break;
            }
        }
    }

    let end_line = (best_start + window_size).min(lines.len());
    let mut snippet = String::new();

    for idx in best_start..end_line {
        let line = lines[idx];
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let line_num = idx + 1;
        let truncated = if trimmed.chars().count() > 110 {
            let s: String = trimmed.chars().take(110).collect();
            format!("{}…", s)
        } else {
            trimmed.to_string()
        };

        let highlighted = highlight_terms(&truncated, &query_terms, use_color);
        if use_color {
            snippet.push_str(&format!("  \x1b[90m{:>4} │\x1b[0m {}\n", line_num, highlighted));
        } else {
            snippet.push_str(&format!("  {:>4} │ {}\n", line_num, highlighted));
        }
    }

    if snippet.is_empty() {
        None
    } else {
        Some(snippet)
    }
}

fn highlight_terms(line: &str, terms: &[String], use_color: bool) -> String {
    if !use_color || terms.is_empty() {
        return line.to_string();
    }

    let chars: Vec<char> = line.chars().collect();
    let mut out = String::with_capacity(line.len() + 32);
    let mut i = 0;

    while i < chars.len() {
        let mut matched_len = 0;
        for term in terms {
            let term_chars: Vec<char> = term.chars().collect();
            if i + term_chars.len() <= chars.len() {
                let slice = &chars[i..i + term_chars.len()];
                let matches = slice
                    .iter()
                    .zip(&term_chars)
                    .all(|(c1, c2)| c1.to_lowercase().eq(c2.to_lowercase()));
                if matches && term_chars.len() > matched_len {
                    matched_len = term_chars.len();
                }
            }
        }

        if matched_len > 0 {
            out.push_str("\x1b[1;33m");
            for c in &chars[i..i + matched_len] {
                out.push(*c);
            }
            out.push_str("\x1b[0m");
            i += matched_len;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }

    out
}
