mod counter;
mod language;
mod output;
mod remote;
mod walker;

use clap::Parser;
use rayon::prelude::*;
use std::collections::HashMap;
use std::io::{BufReader, Cursor};
use std::path::{Path, PathBuf};

use counter::{count_lines, count_lines_reader, LineStats};
use language::{detect_language, get_language_configs};
use output::{print_json, print_table, LanguageStats};
use walker::FileWalker;

type Aggregate = (HashMap<String, LanguageStats>, LineStats, usize);

#[derive(Parser, Debug)]
struct Args {
    #[arg(default_value = ".")]
    path: PathBuf,

    #[arg(long)]
    link: Option<String>,

    #[arg(long)]
    git_ref: Option<String>,

    #[arg(long)]
    github_token: Option<String>,

    #[arg(short = 'H', long)]
    hidden: bool,

    #[arg(long)]
    no_ignore: bool,

    #[arg(short, long)]
    json: bool,

    #[arg(short, long, value_delimiter = ',')]
    extensions: Option<Vec<String>>,

    #[arg(short = 'x', long, value_delimiter = ',')]
    exclude: Option<Vec<String>>,
}

fn should_include_path(path: &Path, args: &Args) -> bool {
    if let Some(exts) = args.extensions.as_ref() {
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            return false;
        };
        if !exts.iter().any(|e| e.eq_ignore_ascii_case(ext)) {
            return false;
        }
    }

    if let Some(excludes) = args.exclude.as_ref() {
        let path_str = path.to_string_lossy();
        if excludes.iter().any(|ex| path_str.contains(ex)) {
            return false;
        }
    }

    true
}

fn is_probably_binary(bytes: &[u8]) -> bool {
    const PROBE_BYTES: usize = 8192;
    bytes.iter().take(PROBE_BYTES).any(|&b| b == 0)
}

fn add_file_stats(
    map: &mut HashMap<String, LanguageStats>,
    total: &mut LineStats,
    files_count: &mut usize,
    lang_name: &str,
    file_stats: &LineStats,
) {
    let slot = map.entry(lang_name.to_string()).or_insert(LanguageStats {
        files: 0,
        stats: LineStats::default(),
    });

    slot.files += 1;
    slot.stats.add(file_stats);
    total.add(file_stats);
    *files_count += 1;
}

fn reduce_aggregates(
    (mut map_a, mut total_a, mut files_a): Aggregate,
    (map_b, total_b, files_b): Aggregate,
) -> Aggregate {
    for (lang, stats_b) in map_b {
        let slot = map_a.entry(lang).or_insert(LanguageStats {
            files: 0,
            stats: LineStats::default(),
        });
        slot.files += stats_b.files;
        slot.stats.add(&stats_b.stats);
    }

    total_a.add(&total_b);
    files_a += files_b;
    (map_a, total_a, files_a)
}

fn main() {
    let args = Args::parse();
    let lang_configs = get_language_configs();

    let (stats_map, total, files_count): Aggregate = if let Some(link) = args.link.as_deref() {
        const REMOTE_BATCH_BYTES: usize = 128 * 1024 * 1024;
        let mut global: Aggregate = (HashMap::new(), LineStats::default(), 0);

        remote::stream_github_repo_in_memory(
            link,
            args.git_ref.as_deref(),
            args.github_token.as_deref(),
            REMOTE_BATCH_BYTES,
            |batch| {
                let batch_agg = batch
                    .into_par_iter()
                    .filter(|file| should_include_path(&file.rel_path, &args))
                    .filter(|file| !is_probably_binary(&file.bytes))
                    .fold(
                        || (HashMap::<String, LanguageStats>::new(), LineStats::default(), 0usize),
                        |(mut local_map, mut local_total, mut local_files), file| {
                            let extension = file
                                .rel_path
                                .extension()
                                .and_then(|e| e.to_str())
                                .map(|e| e.to_ascii_lowercase());

                            let lang_config = extension
                                .as_deref()
                                .and_then(|ext| detect_language(ext, &lang_configs));

                            let lang_name = lang_config.map(|c| c.name).unwrap_or("Other");

                            let reader = BufReader::new(Cursor::new(file.bytes.as_slice()));
                            if let Ok(file_stats) = count_lines_reader(reader, lang_config) {
                                add_file_stats(
                                    &mut local_map,
                                    &mut local_total,
                                    &mut local_files,
                                    lang_name,
                                    &file_stats,
                                );
                            }

                            (local_map, local_total, local_files)
                        },
                    )
                    .reduce(
                        || (HashMap::<String, LanguageStats>::new(), LineStats::default(), 0usize),
                        reduce_aggregates,
                    );

                global = reduce_aggregates(std::mem::take(&mut global), batch_agg);
            },
        )
        .unwrap_or_else(|e| {
            eprintln!("Error fetching repository: {e}");
            std::process::exit(1);
        });

        global
    } else {
        if !args.path.exists() {
            eprintln!("Error: Path '{}' does not exist", args.path.display());
            std::process::exit(1);
        }

        let walker = FileWalker::new(!args.no_ignore, args.hidden);
        let files: Vec<_> = walker
            .walk(&args.path)
            .filter(|entry| should_include_path(entry.path(), &args))
            .collect();

        files
            .par_iter()
            .fold(
                || (HashMap::<String, LanguageStats>::new(), LineStats::default(), 0usize),
                |(mut local_map, mut local_total, mut local_files), entry| {
                    let path = entry.path();

                    let extension = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e.to_ascii_lowercase());

                    let lang_config = extension
                        .as_deref()
                        .and_then(|ext| detect_language(ext, &lang_configs));

                    let lang_name = lang_config.map(|c| c.name).unwrap_or("Other");

                    if let Ok(file_stats) = count_lines(path, lang_config) {
                        add_file_stats(
                            &mut local_map,
                            &mut local_total,
                            &mut local_files,
                            lang_name,
                            &file_stats,
                        );
                    }

                    (local_map, local_total, local_files)
                },
            )
            .reduce(
                || (HashMap::<String, LanguageStats>::new(), LineStats::default(), 0usize),
                reduce_aggregates,
            )
    };

    if args.json {
        print_json(&stats_map, &total, files_count);
    } else {
        print_table(&stats_map, &total, files_count);
    }
}
