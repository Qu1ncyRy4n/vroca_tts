//! Markdown cleanup and sentence splitting, ported from `python_impl/daemon.py`
//! (`clean_markdown`, `sentences`, `_is_abbrev`) without a regex dependency,
//! since `vroca-core` is serde-only (DEC-2). Behavior matches Python on the
//! parity table in `tests/text_parity.rs`, including its quirks: the image
//! rule never fires because the link rule runs first, so `![alt](url)`
//! speaks as `!alt`; and `_`/`*` emphasis applies inside identifiers.

const ABBREV: [&str; 18] = [
    "mr", "mrs", "ms", "dr", "st", "vs", "prof", "sr", "jr", "e.g", "i.e", "etc", "fig", "no",
    "approx", "inc", "ltd", "co",
];
const EMPHASIS: [&str; 5] = ["**", "__", "*", "_", "~~"];

/// Split text into sentences the way the Python daemon does.
pub fn sentences(text: &str) -> Vec<String> {
    let text = clean_markdown(text);
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while let Some(&c) = chars.get(i) {
        if !is_terminal(c) {
            i += 1;
            continue;
        }
        // BOUNDARY = [.!?]+["')\]]*(?=\s). Starting inside a run cannot
        // succeed where the run's start failed, so runs are taken whole.
        let run_start = i;
        let mut j = i;
        while chars.get(j).copied().is_some_and(is_terminal) {
            j += 1;
        }
        while chars.get(j).copied().is_some_and(is_closer) {
            j += 1;
        }
        if chars.get(j).is_some_and(|c| c.is_whitespace()) && !is_abbrev(&chars, run_start + 1) {
            push_trimmed(&mut out, chars.get(start..j));
            start = j;
        }
        i = j;
    }
    push_trimmed(&mut out, chars.get(start..));
    out
}

fn push_trimmed(out: &mut Vec<String>, part: Option<&[char]>) {
    let chunk: String = part.unwrap_or_default().iter().collect();
    let chunk = chunk.trim();
    if !chunk.is_empty() {
        out.push(chunk.to_string());
    }
}

fn is_terminal(c: char) -> bool {
    matches!(c, '.' | '!' | '?')
}

fn is_closer(c: char) -> bool {
    matches!(c, '"' | '\'' | ')' | ']')
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// True if the period at `end` terminates an abbreviation or an initial.
fn is_abbrev(chars: &[char], end: usize) -> bool {
    let head: String = chars.get(..end).unwrap_or_default().iter().collect();
    let head = head.trim_end_matches(['.', '!', '?', '"', '\'', ')', ']']);
    let word = head
        .rsplit(|c: char| c.is_whitespace() || matches!(c, '(' | '[' | '"' | '\''))
        .next()
        .unwrap_or_default();
    if ABBREV.contains(&word.to_lowercase().as_str()) {
        return true;
    }
    // INITIAL = \b[A-Z]$
    let mut tail = head.chars().rev();
    match (tail.next(), tail.next()) {
        (Some(last), before) => last.is_ascii_uppercase() && !before.is_some_and(is_word_char),
        (None, _) => false,
    }
}

/// Normalize markdown so headings, lists, emphasis, and code read naturally.
pub fn clean_markdown(text: &str) -> String {
    let text = replace_fences(text);
    let text = strip_inline_code(&text);
    let text = strip_links(&text);
    let text = strip_emphasis(&text);
    let text: Vec<String> = text.split('\n').map(clean_line).collect();
    text.join("\n")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// ```[\s\S]*?``` → " Code snippet. "
fn replace_fences(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some((before, after)) = rest.split_once("```") {
        let Some((_code, tail)) = after.split_once("```") else {
            break;
        };
        out.push_str(before);
        out.push_str(" Code snippet. ");
        rest = tail;
    }
    out.push_str(rest);
    out
}

/// `([^`]+)` → its content.
fn strip_inline_code(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while let Some(&c) = chars.get(i) {
        if c == '`' {
            let close = (i + 1..chars.len()).find(|&k| chars.get(k) == Some(&'`'));
            if let Some(k) = close.filter(|&k| k > i + 1) {
                out.extend(chars.get(i + 1..k).unwrap_or_default());
                i = k + 1;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

/// \[([^\]]+)\]\([^)]+\) → the link text.
fn strip_links(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while let Some(&c) = chars.get(i) {
        if c == '[' {
            if let Some((label_end, end)) = link_at(&chars, i) {
                out.extend(chars.get(i + 1..label_end).unwrap_or_default());
                i = end;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

/// For `[` at `open`, the index of the closing `]` and one past the `)`.
fn link_at(chars: &[char], open: usize) -> Option<(usize, usize)> {
    let close = (open + 1..chars.len()).find(|&k| chars.get(k) == Some(&']'))?;
    if close == open + 1 || chars.get(close + 1) != Some(&'(') {
        return None;
    }
    let paren = (close + 2..chars.len()).find(|&k| chars.get(k) == Some(&')'))?;
    (paren > close + 2).then_some((close, paren + 1))
}

/// (\*\*|__|\*|_|~~)(.*?)\1 → the inner text, within one line.
fn strip_emphasis(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    'scan: while let Some(&c) = chars.get(i) {
        for marker in EMPHASIS {
            let m: Vec<char> = marker.chars().collect();
            if !starts_with(&chars, i, &m) {
                continue;
            }
            let mut k = i + m.len();
            while let Some(&ch) = chars.get(k) {
                if starts_with(&chars, k, &m) {
                    out.extend(chars.get(i + m.len()..k).unwrap_or_default());
                    i = k + m.len();
                    continue 'scan;
                }
                if ch == '\n' {
                    break;
                }
                k += 1;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

fn starts_with(chars: &[char], at: usize, needle: &[char]) -> bool {
    chars
        .get(at..at + needle.len())
        .is_some_and(|window| window == needle)
}

/// The four line rules, applied in Python's order: heading, bullet,
/// numbered item, quote. Each turns the line into its content plus ".".
fn clean_line(line: &str) -> String {
    let mut line = line.to_string();
    for rule in [heading, bullet, numbered, quote] {
        if let Some(content) = rule(line.trim_start()) {
            line = format!("{content}.");
        }
    }
    line
}

/// The content after `marker_len` chars, if a whitespace run follows.
fn after_marker(line: &str, marker_len: usize) -> Option<&str> {
    let (_, rest) = line.split_at_checked(marker_len)?;
    rest.starts_with(char::is_whitespace)
        .then(|| rest.trim_start())
}

fn heading(line: &str) -> Option<&str> {
    let hashes = line.chars().take_while(|&c| c == '#').count();
    if (1..=6).contains(&hashes) {
        after_marker(line, hashes)
    } else {
        None
    }
}

fn bullet(line: &str) -> Option<&str> {
    if line.starts_with(['-', '*', '+']) {
        after_marker(line, 1)
    } else {
        None
    }
}

fn numbered(line: &str) -> Option<&str> {
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    if digits > 0 && line.get(digits..digits + 1) == Some(".") {
        after_marker(line, digits + 1)
    } else {
        None
    }
}

fn quote(line: &str) -> Option<&str> {
    if line.starts_with('>') {
        after_marker(line, 1)
    } else {
        None
    }
}
