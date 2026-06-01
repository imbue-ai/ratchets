//! Git diff support for `ratchets check --since <ref>`
//!
//! This module shells out to `git` (via `std::process::Command`) to enumerate
//! the files changed between the working tree and a given ref. The result is
//! intersected with the file walker's output so that `ratchets check` runs
//! only on the files the user actually touched.
//!
//! `git2` is intentionally not used: the project does not currently depend on
//! it and the workflow only needs `git diff --name-only` and
//! `git rev-parse --show-toplevel`.

use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Command;
use thiserror::Error;

/// Errors that can occur while resolving the changed-file set for `--since`.
#[derive(Debug, Error)]
pub enum GitDiffError {
    /// The `git` executable could not be invoked at all.
    #[error("Failed to invoke git: {0}")]
    Spawn(#[source] std::io::Error),

    /// The current working directory is not inside a git repository.
    #[error(
        "Not a git repository (or git is unavailable). Run `ratchets check --since` from within a git working tree."
    )]
    NotARepo,

    /// `git diff` rejected the supplied ref (e.g. unknown branch or commit).
    #[error("git diff {reference} failed: {stderr}")]
    BadRef { reference: String, stderr: String },
}

/// Returns the absolute paths of files that differ between the working tree
/// and `reference`, as reported by `git diff <reference> --name-only`.
///
/// The set contains absolute, non-canonicalized paths anchored at the git
/// repository root (`git rev-parse --show-toplevel`). Files that git lists but
/// no longer exist on disk (e.g. files deleted since `reference`) are included
/// here and filtered out downstream by the file walker, which only yields
/// extant files.
///
/// # Errors
///
/// - [`GitDiffError::Spawn`] if `git` cannot be executed.
/// - [`GitDiffError::NotARepo`] if the working directory is not inside a git
///   repository.
/// - [`GitDiffError::BadRef`] if `git diff` rejects `reference`.
pub fn changed_files_since(reference: &str) -> Result<HashSet<PathBuf>, GitDiffError> {
    let repo_root = git_repo_root()?;

    let diff_output = Command::new("git")
        .args(["diff", reference, "--name-only"])
        .output()
        .map_err(GitDiffError::Spawn)?;

    if !diff_output.status.success() {
        let stderr = String::from_utf8_lossy(&diff_output.stderr)
            .trim()
            .to_string();
        return Err(GitDiffError::BadRef {
            reference: reference.to_string(),
            stderr,
        });
    }

    let stdout = String::from_utf8_lossy(&diff_output.stdout);
    let changed = stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| repo_root.join(line))
        .collect();

    Ok(changed)
}

/// Returns the absolute path to the git repository root by shelling out to
/// `git rev-parse --show-toplevel`.
fn git_repo_root() -> Result<PathBuf, GitDiffError> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(GitDiffError::Spawn)?;

    if !output.status.success() {
        return Err(GitDiffError::NotARepo);
    }

    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if raw.is_empty() {
        return Err(GitDiffError::NotARepo);
    }
    Ok(PathBuf::from(raw))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_diff_error_display_not_a_repo() {
        let err = GitDiffError::NotARepo;
        assert!(err.to_string().contains("Not a git repository"));
    }

    #[test]
    fn test_git_diff_error_display_bad_ref() {
        let err = GitDiffError::BadRef {
            reference: "does-not-exist".to_string(),
            stderr: "unknown revision".to_string(),
        };
        let s = err.to_string();
        assert!(s.contains("does-not-exist"));
        assert!(s.contains("unknown revision"));
    }
}
