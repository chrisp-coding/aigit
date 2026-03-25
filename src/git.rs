use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

#[allow(dead_code)]
pub struct BlameEntry {
    pub line_start: u32,
    pub line_end: u32,
    pub commit_hash: String,
    pub author: String,
    pub timestamp: i64,
}

fn open_repo(base: &Path) -> Result<Option<git2::Repository>> {
    match git2::Repository::discover(base) {
        Ok(repo) => Ok(Some(repo)),
        Err(e) if e.code() == git2::ErrorCode::NotFound => Ok(None),
        Err(e) => Err(e).context("failed to open Git repository"),
    }
}

pub fn get_current_hash(base: &Path) -> Result<Option<String>> {
    let repo = match open_repo(base)? {
        Some(r) => r,
        None => return Ok(None),
    };

    let head = match repo.head() {
        Ok(h) => h,
        Err(e) if e.code() == git2::ErrorCode::UnbornBranch => return Ok(None),
        Err(e) => return Err(e).context("failed to read HEAD"),
    };

    let oid = head.peel_to_commit()
        .context("HEAD does not point to a commit")?
        .id();

    Ok(Some(oid.to_string()))
}

pub fn get_repo_root(base: &Path) -> Result<Option<PathBuf>> {
    let repo = match open_repo(base)? {
        Some(r) => r,
        None => return Ok(None),
    };

    let workdir = repo.workdir()
        .map(|p| p.to_path_buf())
        .or_else(|| Some(repo.path().to_path_buf()));

    Ok(workdir)
}

/// Returns the Git hash of the parent of HEAD (i.e. HEAD~1), if it exists.
pub fn get_parent_hash(base: &Path) -> Result<Option<String>> {
    let repo = match open_repo(base)? {
        Some(r) => r,
        None => return Ok(None),
    };

    let head = match repo.head() {
        Ok(h) => h,
        Err(e) if e.code() == git2::ErrorCode::UnbornBranch => return Ok(None),
        Err(e) => return Err(e).context("failed to read HEAD"),
    };

    let commit = head.peel_to_commit()
        .context("HEAD does not point to a commit")?;

    if commit.parent_count() == 0 {
        return Ok(None);
    }

    let hash = commit.parent(0).context("failed to read parent commit")?.id().to_string();
    Ok(Some(hash))
}

/// Returns the Unix timestamp (seconds) of the parent of HEAD, if one exists.
/// Used by the post-commit hook to bound the search window for unlinked aigit commits.
pub fn get_parent_timestamp(base: &Path) -> Result<Option<i64>> {
    let repo = match open_repo(base)? {
        Some(r) => r,
        None => return Ok(None),
    };

    let head = match repo.head() {
        Ok(h) => h,
        Err(e) if e.code() == git2::ErrorCode::UnbornBranch => return Ok(None),
        Err(e) => return Err(e).context("failed to read HEAD"),
    };

    let commit = head.peel_to_commit().context("HEAD is not a commit")?;
    if commit.parent_count() == 0 {
        return Ok(None);
    }

    let parent = commit.parent(0).context("failed to read parent commit")?;
    Ok(Some(parent.time().seconds()))
}

/// Returns the commit message of HEAD, if available.
pub fn get_head_commit_message(base: &Path) -> Result<Option<String>> {
    let repo = match open_repo(base)? {
        Some(r) => r,
        None => return Ok(None),
    };

    let head = match repo.head() {
        Ok(h) => h,
        Err(e) if e.code() == git2::ErrorCode::UnbornBranch => return Ok(None),
        Err(e) => return Err(e).context("failed to read HEAD"),
    };

    let commit = head.peel_to_commit().context("HEAD is not a commit")?;
    Ok(commit.message().map(|s| s.trim().to_string()))
}

/// Returns the list of Git commit hashes that modified a given file, newest first.
pub fn get_commits_for_file(base: &Path, path: &Path) -> Result<Vec<String>> {
    let repo = match open_repo(base)? {
        Some(r) => r,
        None => return Ok(vec![]),
    };

    let mut revwalk = repo.revwalk().context("failed to create revwalk")?;
    if revwalk.push_head().is_err() {
        return Ok(vec![]);
    }

    let mut hashes = vec![];

    for oid_result in revwalk {
        let oid = match oid_result {
            Ok(o) => o,
            Err(_) => break,
        };
        let commit = match repo.find_commit(oid) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let tree = match commit.tree() {
            Ok(t) => t,
            Err(_) => continue,
        };

        let touched = if commit.parent_count() == 0 {
            tree.get_path(path).is_ok()
        } else {
            let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());
            let diff = repo.diff_tree_to_tree(
                parent_tree.as_ref(),
                Some(&tree),
                None,
            );
            match diff {
                Ok(d) => d.deltas().any(|delta| {
                    delta.new_file().path() == Some(path)
                        || delta.old_file().path() == Some(path)
                }),
                Err(_) => false,
            }
        };

        if touched {
            hashes.push(oid.to_string());
        }
    }

    Ok(hashes)
}

/// Returns paths of files that are modified in the working tree or index relative to HEAD.
pub fn get_modified_files(base: &Path) -> Result<Vec<String>> {
    let repo = match open_repo(base)? {
        Some(r) => r,
        None => return Ok(vec![]),
    };

    let mut paths = std::collections::HashSet::new();

    // Staged changes (index vs HEAD)
    let head_tree = repo.head().ok()
        .and_then(|h| h.peel_to_commit().ok())
        .and_then(|c| c.tree().ok());
    let index = repo.index().context("failed to read index")?;
    let staged_diff = repo.diff_tree_to_index(head_tree.as_ref(), Some(&index), None)
        .context("failed to diff HEAD to index")?;
    for delta in staged_diff.deltas() {
        if let Some(p) = delta.new_file().path().or_else(|| delta.old_file().path()) {
            paths.insert(p.to_string_lossy().into_owned());
        }
    }

    // Unstaged changes (index vs working tree)
    let unstaged_diff = repo.diff_index_to_workdir(Some(&index), None)
        .context("failed to diff index to workdir")?;
    for delta in unstaged_diff.deltas() {
        if let Some(p) = delta.new_file().path().or_else(|| delta.old_file().path()) {
            paths.insert(p.to_string_lossy().into_owned());
        }
    }

    let mut result: Vec<String> = paths.into_iter().collect();
    result.sort();
    Ok(result)
}

pub fn get_file_blame(base: &Path, path: &Path) -> Result<Vec<BlameEntry>> {
    let repo = match open_repo(base)? {
        Some(r) => r,
        None => return Ok(vec![]),
    };

    let blame = match repo.blame_file(path, None) {
        Ok(b) => b,
        Err(_) => return Ok(vec![]),
    };

    let mut entries: Vec<BlameEntry> = Vec::new();

    for hunk in blame.iter() {
        let sig = hunk.final_signature();
        let commit_id = hunk.final_commit_id();

        let author = sig.name().unwrap_or("unknown").to_string();
        let timestamp = sig.when().seconds();
        let commit_hash = commit_id.to_string();

        let line_start = hunk.final_start_line() as u32;
        let line_end = (hunk.final_start_line() + hunk.lines_in_hunk().saturating_sub(1)) as u32;

        entries.push(BlameEntry {
            line_start,
            line_end,
            commit_hash,
            author,
            timestamp,
        });
    }

    Ok(entries)
}
