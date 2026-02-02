use crate::counter::LineStats;
use colored::*;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct LanguageStats {
    pub files: usize,
    pub stats: LineStats,
}

pub fn print_table(
    stats: &HashMap<String, LanguageStats>,
    total_stats: &LineStats,
    total_files: usize,
) {
    println!();
    println!("{:─<80}", "".bright_blue());
    println!(
        "{:<15} {:>10} {:>12} {:>12} {:>12} {:>12}",
        "Language".bold().cyan(),
        "Files".bold().cyan(),
        "Total".bold().cyan(),
        "Code".bold().cyan(),
        "Comments".bold().cyan(),
        "Blank".bold().cyan()
    );
    println!("{:─<80}", "".bright_blue());

    // Sort by code lines (descending)
    let mut sorted: Vec<_> = stats.iter().collect();
    sorted.sort_by(|a, b| b.1.stats.code.cmp(&a.1.stats.code));

    for (lang, lang_stats) in sorted {
        println!(
            "{:<15} {:>10} {:>12} {:>12} {:>12} {:>12}",
            lang.green(),
            lang_stats.files.to_string().yellow(),
            lang_stats.stats.total.to_string().white(),
            lang_stats.stats.code.to_string().bright_green(),
            lang_stats.stats.comments.to_string().bright_blue(),
            lang_stats.stats.blank.to_string().dimmed()
        );
    }

    println!("{:─<80}", "".bright_blue());
    println!(
        "{:<15} {:>10} {:>12} {:>12} {:>12} {:>12}",
        "Total".bold().magenta(),
        total_files.to_string().bold().yellow(),
        total_stats.total.to_string().bold().white(),
        total_stats.code.to_string().bold().bright_green(),
        total_stats.comments.to_string().bold().bright_blue(),
        total_stats.blank.to_string().bold().dimmed()
    );
    println!("{:─<80}", "".bright_blue());
    println!();
}

pub fn print_json(
    stats: &HashMap<String, LanguageStats>,
    total_stats: &LineStats,
    total_files: usize,
) {
    use serde::Serialize;

    #[derive(Serialize)]
    struct JsonOutput {
        languages: HashMap<String, JsonLanguageStats>,
        total: JsonTotalStats,
    }

    #[derive(Serialize)]
    struct JsonLanguageStats {
        files: usize,
        total: usize,
        code: usize,
        comments: usize,
        blank: usize,
    }

    #[derive(Serialize)]
    struct JsonTotalStats {
        files: usize,
        total: usize,
        code: usize,
        comments: usize,
        blank: usize,
    }

    let languages: HashMap<_, _> = stats
        .iter()
        .map(|(lang, ls)| {
            (
                lang.clone(),
                JsonLanguageStats {
                    files: ls.files,
                    total: ls.stats.total,
                    code: ls.stats.code,
                    comments: ls.stats.comments,
                    blank: ls.stats.blank,
                },
            )
        })
        .collect();

    let output = JsonOutput {
        languages,
        total: JsonTotalStats {
            files: total_files,
            total: total_stats.total,
            code: total_stats.code,
            comments: total_stats.comments,
            blank: total_stats.blank,
        },
    };

    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}
