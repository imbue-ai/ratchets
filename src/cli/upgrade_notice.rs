//! Embedded upgrade notice rendered on `ConfigError::UnsupportedVersion`.
//!
//! Phase 1 of the ratchet-sets plan only ships a placeholder string here. The
//! canonical doc (`docs/upgrade-v1-to-v2.md`) is written in Phase 5 (bead
//! `code-rs-p5`) and will be embedded via `include_str!()` at that point. Until
//! then the placeholder keeps the `--upgrade` plumbing functional and points
//! curious users at the bead where the canonical content will land.

/// Embedded upgrade notice text printed to stderr when the loaded
/// `ratchets.toml` declares a version other than `"2"`.
///
/// Phase 1 placeholder — the canonical content arrives in bead `code-rs-p5`.
pub const UPGRADE_NOTICE: &str = "\
ratchets configuration: unsupported schema version.

The library now only accepts `version = \"2\"` in ratchets.toml.

A canonical upgrade guide will ship in docs/upgrade-v1-to-v2.md (bead code-rs-p5).
For now: bump `[ratchets].version` to \"2\" and migrate any
`[rules].rule-id = true | false` lines to `enabled_ratchets` /
`disabled_ratchets` arrays at the top of the file.
";

/// Print [`UPGRADE_NOTICE`] to stderr. Called by every CLI subcommand that
/// loads `ratchets.toml` when it encounters
/// [`crate::error::ConfigError::UnsupportedVersion`].
pub fn print_to_stderr() {
    eprintln!("{}", UPGRADE_NOTICE);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upgrade_notice_is_non_empty() {
        assert!(!UPGRADE_NOTICE.trim().is_empty());
    }

    #[test]
    fn upgrade_notice_mentions_version_2() {
        assert!(
            UPGRADE_NOTICE.contains("version = \"2\""),
            "notice should tell users the new required version"
        );
    }
}
