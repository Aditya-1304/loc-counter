use flate2::read::GzDecoder;
use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, AUTHORIZATION, USER_AGENT};
use std::error::Error;
use std::io::Read;
use std::path::{Path, PathBuf};
use tar::Archive;
use url::Url;

type AnyError = Box<dyn Error + Send + Sync>;

#[derive(Debug)]
pub struct RemoteFile {
    pub rel_path: PathBuf,
    pub bytes: Vec<u8>,
}

pub fn stream_github_repo_in_memory<F>(
    repo_url: &str,
    git_ref: Option<&str>,
    token: Option<&str>,
    mut on_file: F,
) -> Result<(), AnyError>
where
    F: FnMut(RemoteFile) -> Result<(), AnyError>,
{
    let (owner, repo) = parse_github_url(repo_url)?;

    let endpoint = match git_ref {
        Some(r) => format!("https://api.github.com/repos/{owner}/{repo}/tarball/{r}"),
        None => format!("https://api.github.com/repos/{owner}/{repo}/tarball"),
    };

    let client = Client::builder().build()?;
    let mut req = client
        .get(&endpoint)
        .header(USER_AGENT, "loc_counter")
        .header(ACCEPT, "application/vnd.github+json");

    if let Some(t) = token {
        req = req.header(AUTHORIZATION, format!("Bearer {t}"));
    }

    let resp = req.send()?.error_for_status()?;
    if let Some(len) = resp.content_length() {
        const MAX_ARCHIVE_BYTES: u64 = 500 * 1024 * 1024;
        if len > MAX_ARCHIVE_BYTES {
            return Err(format!("Repository archive too large: {len} bytes").into());
        }
    }

    let decoder = GzDecoder::new(resp);
    let mut archive = Archive::new(decoder);

    for entry_result in archive.entries()? {
        let mut entry = entry_result?;

        if !entry.header().entry_type().is_file() {
            continue;
        }

        let full_path = entry.path()?.into_owned();
        let rel_path = strip_archive_root(&full_path);
        if rel_path.as_os_str().is_empty() {
            continue;
        }

        let mut bytes = Vec::with_capacity(entry.size().min(8 * 1024 * 1024) as usize);
        entry.read_to_end(&mut bytes)?;

        on_file(RemoteFile { rel_path, bytes })?;
    }

    Ok(())
}

fn strip_archive_root(path: &Path) -> PathBuf {
    let mut components = path.components();
    let _ = components.next();
    components.as_path().to_path_buf()
}

fn parse_github_url(input: &str) -> Result<(String, String), AnyError> {
    let url = Url::parse(input)?;
    if url.domain() != Some("github.com") {
        return Err("Only github.com URLs are supported for --link right now".into());
    }

    let mut parts = url.path().trim_matches('/').split('/');
    let owner = parts.next().ok_or("Missing owner in github url")?;
    let repo_raw = parts.next().ok_or("Missing repo name in github url")?;
    let repo = repo_raw.strip_suffix(".git").unwrap_or(repo_raw);

    if owner.is_empty() || repo.is_empty() {
        return Err("Invalid github repository url".into());
    }

    Ok((owner.to_string(), repo.to_string()))
}
