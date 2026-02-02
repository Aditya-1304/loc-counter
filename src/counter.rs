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

pub fn count_lines(path: &Path, lang_config: Option<LanguageConfig>) -> Result<LineStats> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let mut stats = LineStats::default();
    let mut in_block_comment = false;

    let (line_comment, block_comment) = match lang_config {
        Some(c) => (c.line_comment, c.block_comment),
        None => (None, None),
    };

    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();
        stats.total += 1;

        if trimmed.is_empty() {
            stats.blank += 1;
            continue;
        }

        // handle block comments
        if let Some((start, end)) = block_comment {
            if in_block_comment {
                stats.comments += 1;
                if trimmed.contains(end) {
                    in_block_comment = false;
                }
                continue;
            }

            if trimmed.starts_with(start) {
                stats.comments += 1;
                if !trimmed.contains(end) || trimmed.ends_with(start) {
                    in_block_comment = true;
                }
                continue;
            }
        }

        // handle line comments
        if let Some(comment_prefix) = line_comment {
            if trimmed.starts_with(comment_prefix) {
                stats.comments += 1;
                continue;
            }
        }
        stats.code += 1;
    }
    Ok(stats)
}
