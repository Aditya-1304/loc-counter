mod counter;
mod language;
mod output;
mod remote;
mod walker;

use clap::Parser;
use crossbeam_channel::{bounded, unbounded};
use rayon::prelude::*;
use std::borrow::Cow;
use std::collections::HashMap;
use std::error::Error;
use std::io::{BufReader, Cursor};
use std::path::{Path, PathBuf};

use counter::{count_lines, count_lines_reader, LineStats};
use language::{detect_language, get_language_configs, LanguageConfig};
use output::{print_json, print_table, LanguageStats};
use walker::FileWalker;

type AnyError = Box<dyn Error + Send + Sync>;
type LangConfigs = HashMap<&'static str, &'static LanguageConfig>;
type StatsMap = HashMap<&'static str, LanguageStats>;
type Aggregate = (StatsMap, LineStats, usize);

const OTHER_LANG: &str = "Other";
const REMOTE_QUEUE_MULTIPLIER: usize = 8;

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

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), AnyError> {
    let args = Args::parse();
    let lang_configs = get_language_configs();

    let (stats_map, total, files_count) = if args.link.is_some() {
        count_remote_repo(&args, &lang_configs)?
    } else {
        if !args.path.exists() {
            return Err(format!("Path '{}' does not exist", args.path.display()).into());
        }
        count_local_repo(&args, &lang_configs)
    };

    if args.json {
        print_json(&stats_map, &total, files_count);
    } else {
        print_table(&stats_map, &total, files_count);
    }

    Ok(())
}

fn empty_aggregate() -> Aggregate {
    (HashMap::new(), LineStats::default(), 0)
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

fn normalized_extension(path: &Path) -> Option<Cow<'_, str>> {
    let ext = path.extension().and_then(|e| e.to_str())?;
    if ext.bytes().any(|b| b.is_ascii_uppercase()) {
        Some(Cow::Owned(ext.to_ascii_lowercase()))
    } else {
        Some(Cow::Borrowed(ext))
    }
}

fn detect_language_for_path(
    path: &Path,
    configs: &LangConfigs,
) -> Option<&'static LanguageConfig> {
    let ext = normalized_extension(path)?;
    detect_language(ext.as_ref(), configs)
}

fn is_probably_binary(bytes: &[u8]) -> bool {
    const PROBE_BYTES: usize = 8192;
    bytes.iter().take(PROBE_BYTES).any(|&b| b == 0)
}

fn add_file_stats(
    map: &mut StatsMap,
    total: &mut LineStats,
    files_count: &mut usize,
    lang_name: &'static str,
    file_stats: &LineStats,
) {
    let slot = map.entry(lang_name).or_insert(LanguageStats {
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

fn process_disk_file(local: &mut Aggregate, path: &Path, lang_configs: &LangConfigs) {
    let lang_config = detect_language_for_path(path, lang_configs);
    let lang_name = lang_config.map(|c| c.name).unwrap_or(OTHER_LANG);

    if let Ok(file_stats) = count_lines(path, lang_config) {
        add_file_stats(
            &mut local.0,
            &mut local.1,
            &mut local.2,
            lang_name,
            &file_stats,
        );
    }
}

fn process_memory_file(local: &mut Aggregate, file: remote::RemoteFile, lang_configs: &LangConfigs) {
    let lang_config = detect_language_for_path(&file.rel_path, lang_configs);
    let lang_name = lang_config.map(|c| c.name).unwrap_or(OTHER_LANG);

    let reader = BufReader::new(Cursor::new(file.bytes));
    if let Ok(file_stats) = count_lines_reader(reader, lang_config) {
        add_file_stats(
            &mut local.0,
            &mut local.1,
            &mut local.2,
            lang_name,
            &file_stats,
        );
    }
}

fn count_local_repo(args: &Args, lang_configs: &LangConfigs) -> Aggregate {
    let walker = FileWalker::new(!args.no_ignore, args.hidden);

    walker
        .walk(&args.path)
        .filter(|entry| should_include_path(entry.path(), args))
        .par_bridge()
        .fold(empty_aggregate, |mut local, entry| {
            process_disk_file(&mut local, entry.path(), lang_configs);
            local
        })
        .reduce(empty_aggregate, reduce_aggregates)
}

fn count_remote_repo(args: &Args, lang_configs: &LangConfigs) -> Result<Aggregate, AnyError> {
    let link = args
        .link
        .as_deref()
        .ok_or("internal error: --link branch reached without value")?;

    let workers = rayon::current_num_threads().max(1);
    let queue_capacity = workers * REMOTE_QUEUE_MULTIPLIER;

    let (job_tx, job_rx) = bounded::<remote::RemoteFile>(queue_capacity);
    let (result_tx, result_rx) = unbounded::<Aggregate>();

    let mut producer_result: Result<(), AnyError> = Ok(());

    rayon::scope(|scope| {
        for _ in 0..workers {
            let job_rx = job_rx.clone();
            let result_tx = result_tx.clone();
            let lang_configs = lang_configs;

            scope.spawn(move |_| {
                let mut local = empty_aggregate();

                while let Ok(file) = job_rx.recv() {
                    process_memory_file(&mut local, file, lang_configs);
                }

                let _ = result_tx.send(local);
            });
        }

        drop(result_tx);

        producer_result = remote::stream_github_repo_in_memory(
            link,
            args.git_ref.as_deref(),
            args.github_token.as_deref(),
            |file| {
                if should_include_path(&file.rel_path, args) && !is_probably_binary(&file.bytes) {
                    job_tx
                        .send(file)
                        .map_err(|e| format!("remote worker queue closed: {e}").into())
                } else {
                    Ok(())
                }
            },
        );

        drop(job_tx);
    });

    producer_result?;

    let mut global = empty_aggregate();
    for _ in 0..workers {
        let partial = result_rx
            .recv()
            .map_err(|e| format!("failed to collect worker result: {e}"))?;
        global = reduce_aggregates(global, partial);
    }

    Ok(global)
}
