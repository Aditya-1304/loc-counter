mod counter;
mod language;
mod output;
mod walker;

use clap::Parser;
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use counter::{LineStats, count_lines};
use language::{detect_language, get_language_configs};
use output::{LanguageStats, print_json, print_table};
use walker::FileWalker;

#[derive(Parser, Debug)]
struct Args {
    /// Path to the directory
    #[arg(default_value = ".")]
    path: PathBuf,

    /// hidden files and directories
    #[arg(short = 'H', long)]
    hidden: bool,

    /// Don't respect .gitignore files
    #[arg(long)]
    no_ignore: bool,

    /// Output as JSON
    #[arg(short, long)]
    json: bool,

    /// Only count specific file extensions (comma-separated)
    #[arg(short, long, value_delimiter = ',')]
    extensions: Option<Vec<String>>,

    /// Exclude specific directories (comma-separated)
    #[arg(short = 'x', long, value_delimiter = ',')]
    exclude: Option<Vec<String>>,
}

fn main() {
    let args = Args::parse();

    if !args.path.exists() {
        eprintln!("Error: Path '{}' does not exist", args.path.display());
        std::process::exit(1);
    }

    let lang_configs = get_language_configs();
    let walker = FileWalker::new(!args.no_ignore, args.hidden);

    let files: Vec<_> = walker
        .walk(&args.path)
        .filter(|entry| {
            let path = entry.path();

            if let Some(ref exts) = args.extensions {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if !exts.iter().any(|e| e.eq_ignore_ascii_case(ext)) {
                        return false;
                    }
                } else {
                    return false;
                }
            }

            if let Some(ref excludes) = args.exclude {
                let path_str = path.to_string_lossy();
                if excludes.iter().any(|ex| path_str.contains(ex)) {
                    return false;
                }
            }

            true
        })
        .collect();

    let stats_by_language: Mutex<HashMap<String, LanguageStats>> = Mutex::new(HashMap::new());
    let total_stats: Mutex<LineStats> = Mutex::new(LineStats::default());
    let total_files: Mutex<usize> = Mutex::new(0);

    files.par_iter().for_each(|entry| {
        let path = entry.path();
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase());

        let lang_config = extension
            .as_deref()
            .and_then(|ext| detect_language(ext, &lang_configs));

        let lang_name = lang_config
            .as_ref()
            .map(|c| c.name.to_string())
            .unwrap_or_else(|| "Other".to_string());

        if let Ok(file_stats) = count_lines(path, lang_config.clone()) {
            let mut stats_map = stats_by_language.lock().unwrap();
            let entry = stats_map.entry(lang_name).or_insert(LanguageStats {
                files: 0,
                stats: LineStats::default(),
            });
            entry.files += 1;
            entry.stats.add(&file_stats);

            total_stats.lock().unwrap().add(&file_stats);
            *total_files.lock().unwrap() += 1;
        }
    });

    let stats_map = stats_by_language.into_inner().unwrap();
    let total = total_stats.into_inner().unwrap();
    let files_count = total_files.into_inner().unwrap();

    // Output
    if args.json {
        print_json(&stats_map, &total, files_count);
    } else {
        print_table(&stats_map, &total, files_count);
    }
}
