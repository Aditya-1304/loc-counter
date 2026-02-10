use flate2::read::GzDecoder;
use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, AUTHORIZATION, USER_AGENT};
use std::error::Error;
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use tar::Archive;
use tempfile::TempDir;
use url::Url;

type AnyError = Box<dyn Error + Send + Sync>;

pub struct RepoSource {
  pub root: PathBuf,
  _tmp: TempDir,
}

pub fn fetch_github_repo(
  repo_url: &str,
  git_ref: Option<&str>,
  token: Option<&str>,
) -> Result<RepoSource, AnyError> {
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
    const MAX: u64 = 500 * 1024 * 1024;
    if len > MAX {
      return Err(format!("Repository archive too large: {len} bytes").into());
    }
  }

  let bytes = resp.bytes()?;
  let tmp = TempDir::new()?;

  let decoder = GzDecoder::new(Cursor::new(bytes));
  let mut archive = Archive::new(decoder);
  archive.unpack(tmp.path())?;

  let root = fs::read_dir(tmp.path())?
    .filter_map(Result::ok)
    .map(|e| e.path())
    .find(|p| p.is_dir())
    .ok_or("Could not find extracted repo root directory")?;

  Ok(RepoSource { root, _tmp: tmp })

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