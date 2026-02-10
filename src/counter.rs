use crate::language::LanguageConfig;
use std::fs::File;
use std::io::Result;
use std::io::{BufRead, BufReader};
use std::path::Path;

#[derive(Debug, Default, Clone)]
pub struct LineStats {
    pub total: usize,
    pub code: usize,
    pub comments: usize,
    pub blank: usize,
}

impl LineStats {
    pub fn add(&mut self, other: &LineStats) {
        self.total += other.total;
        self.code += other.code;
        self.comments += other.comments;
        self.blank += other.blank;
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum StringDelimiter {
    Single,       // '
    Double,       // "
    TripleSingle, // '''
    TripleDouble, // """
    Backtick,     // ` (JS template literals)
}

/// Line classification result
#[derive(Debug, Clone, Copy, PartialEq)]
enum LineType {
    Blank,
    Comment,
    Code,
    Mixed,
}

pub fn count_lines(path: &Path, lang_config: Option<&LanguageConfig>) -> Result<LineStats> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);

    if is_probably_binary_prefix(reader.fill_buf()?) {
        return Ok(LineStats::default());
    }

    count_lines_reader(reader, lang_config)
}


pub fn count_lines_reader<R: BufRead>(
    mut reader: R, 
    lang_config: Option<&LanguageConfig>
) -> Result<LineStats> {
    let mut stats = LineStats::default();

    let line_comment = lang_config.and_then(|c| c.line_comment);
    let block_comment = lang_config.and_then(|c| c.block_comment);

    let is_python = lang_config
        .map_or(false, |c| c.name == "Python");

    let is_text = lang_config
        .map_or(false,|c| c.name == "Plain Text" || c.name == "Markdown");

    let mut in_block_comment = false;
    let mut in_string: Option<StringDelimiter> = None;

    let mut line_buf = String::with_capacity(256);

    loop {
        line_buf.clear();
        let n = reader.read_line(&mut line_buf)?;
        if n == 0 {
            break;
        }

        let trimmed = line_buf.trim();
        stats.total += 1;

        if trimmed.is_empty() {
            stats.blank += 1;
            continue;
        }

        if is_text {
            stats.comments += 1;
            continue;
        }

        if in_block_comment {
            stats.comments += 1;
            if let Some((_, end)) = block_comment {
                if let Some(pos) = trimmed.find(end) {
                    let after = &trimmed[pos + end.len()..].trim();
                    if !after.is_empty() && !after.starts_with(line_comment.unwrap_or("")) {
                        stats.comments -= 1;
                        stats.code += 1;
                    }
                    in_block_comment = false;
                }
            }
            continue;
        }

        if let Some(delim) = in_string {
            stats.code += 1;

            let end_delim = match delim {
                StringDelimiter::TripleSingle => "'''",
                StringDelimiter::TripleDouble => "\"\"\"",
                StringDelimiter::Single => "'",
                StringDelimiter::Double => "\"",
                StringDelimiter::Backtick => "`",
            };

            if contains_unescaped(trimmed, end_delim) {
                in_string = None;
            }
            continue;
        }

        let line_type = classify_line(
            trimmed,
            line_comment,
            block_comment,
            is_python,
            &mut in_block_comment,
            &mut in_string,
        );

        match line_type {
            LineType::Blank => stats.blank += 1,
            LineType::Comment => stats.comments += 1,
            LineType::Code | LineType::Mixed => stats.code += 1,
        }
    }

    Ok(stats)
}

/// Classify a line as blank, comment, code, or mixed
fn classify_line(
    line: &str,
    line_comment: Option<&str>,
    block_comment: Option<(&str, &str)>,
    is_python: bool,
    in_block_comment: &mut bool,
    in_string: &mut Option<StringDelimiter>,
) -> LineType {
    let trimmed = line.trim();

    if trimmed.is_empty() {
        return LineType::Blank;
    }

    // For Python, ignore triple-quote "block comments" - they're strings
    let effective_block_comment = if is_python { None } else { block_comment };

    let mut has_code = false;
    let mut has_comment = false;
    let mut current_string: Option<StringDelimiter> = None;
    let mut i = 0;

    while i < trimmed.len() {
        let remaining = &trimmed[i..];

        // Check if we're entering/exiting a string
        if current_string.is_none() {
            // Check for triple-quoted strings first (Python)
            if remaining.starts_with("\"\"\"") {
                current_string = Some(StringDelimiter::TripleDouble);
                has_code = true;
                i += 3;
                continue;
            }
            if remaining.starts_with("'''") {
                current_string = Some(StringDelimiter::TripleSingle);
                has_code = true;
                i += 3;
                continue;
            }

            if remaining.starts_with('"') && !is_escaped(trimmed, i) {
                current_string = Some(StringDelimiter::Double);
                has_code = true;
                i += 1;
                continue;
            }
            if remaining.starts_with('\'') && !is_escaped(trimmed, i) {
                current_string = Some(StringDelimiter::Single);
                has_code = true;
                i += 1;
                continue;
            }
            if remaining.starts_with('`') {
                current_string = Some(StringDelimiter::Backtick);
                has_code = true;
                i += 1;
                continue;
            }

            if let Some((start, _)) = effective_block_comment {
                if remaining.starts_with(start) {
                    if has_code {
                        has_comment = true;
                    } else {
                        has_comment = true;
                    }

                    // Check if block comment ends on same line
                    if let Some((_, end)) = effective_block_comment {
                        let after_start = &remaining[start.len()..];
                        if let Some(end_pos) = after_start.find(end) {
                            // Block comment ends on this line
                            i += start.len() + end_pos + end.len();
                            continue;
                        } else {
                            // Block comment continues to next line
                            *in_block_comment = true;
                            break;
                        }
                    }
                }
            }

            // Check for line comment
            if let Some(comment_prefix) = line_comment {
                if remaining.starts_with(comment_prefix) {
                    has_comment = true;
                    break;
                }
            }

            // Regular code character
            if !remaining.starts_with(char::is_whitespace) {
                has_code = true;
            }
            i += remaining.chars().next().map(|c| c.len_utf8()).unwrap_or(1);
        } else {
            has_code = true;

            let end_delim = match current_string {
                Some(StringDelimiter::TripleDouble) => "\"\"\"",
                Some(StringDelimiter::TripleSingle) => "'''",
                Some(StringDelimiter::Double) => "\"",
                Some(StringDelimiter::Single) => "'",
                Some(StringDelimiter::Backtick) => "`",
                None => {
                    i += remaining.chars().next().map(|c| c.len_utf8()).unwrap_or(1);
                    continue;
                }
            };

            if remaining.starts_with(end_delim) && !is_escaped(trimmed, i) {
                current_string = None;
                i += end_delim.len();
            } else {
                i += remaining.chars().next().map(|c| c.len_utf8()).unwrap_or(1);
            }
        }
    }

    // If we're still in a multi-line string at end of line
    if current_string.is_some() {
        *in_string = current_string;
    }

    // Determine line type
    match (has_code, has_comment) {
        (false, false) => LineType::Blank,
        (false, true) => LineType::Comment,
        (true, false) => LineType::Code,
        (true, true) => LineType::Mixed, 
    }
}

/// Check if position i in string is escaped (preceded by odd number of backslashes)
fn is_escaped(s: &str, pos: usize) -> bool {
    if pos == 0 {
        return false;
    }

    let bytes = s.as_bytes();
    let mut backslash_count = 0;
    let mut i = pos - 1;

    loop {
        if bytes.get(i) == Some(&b'\\') {
            backslash_count += 1;
            if i == 0 {
                break;
            }
            i -= 1;
        } else {
            break;
        }
    }

    backslash_count % 2 == 1
}

/// Check if string contains unescaped delimiter
fn contains_unescaped(s: &str, delim: &str) -> bool {
    let mut i = 0;
    while i < s.len() {
        if let Some(pos) = s[i..].find(delim) {
            let actual_pos = i + pos;
            if !is_escaped(s, actual_pos) {
                return true;
            }
            i = actual_pos + 1;
        } else {
            break;
        }
    }
    false
}

fn is_probably_binary_prefix(bytes: &[u8]) -> bool {
    const PROBE_BYTES: usize = 8192;
    bytes.iter().take(PROBE_BYTES).any(|&b| b == 0)
}
