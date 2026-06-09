//! Embedded upgrade notice rendered on `ConfigError::UnsupportedVersion`.
//!
//! The notice text lives in `docs/upgrade-v1-to-v2.md` (the canonical hand-
//! maintained source) and is embedded into the binary via `include_str!()`.

/// Embedded upgrade notice text printed to stderr when the loaded
/// `ratchets.toml` declares a version other than `"2"`.
///
/// Source of truth: `docs/upgrade-v1-to-v2.md` (markdown, printed raw —
/// readable for humans and easy for LLMs to consume).
pub const UPGRADE_NOTICE: &str = include_str!("../../docs/upgrade-v1-to-v2.md");

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

    #[test]
    fn upgrade_notice_links_to_canonical_url() {
        assert!(
            UPGRADE_NOTICE.contains(
                "https://github.com/imbue-ai/ratchets/blob/main/docs/upgrade-v1-to-v2.md"
            ),
            "notice should point users at the canonical GitHub URL"
        );
    }

    #[test]
    fn upgrade_notice_shows_the_v2_arrays() {
        // The doc must demonstrate the new opt-in arrays so users see the
        // shape they need to migrate to, not just the version bump.
        assert!(UPGRADE_NOTICE.contains("enabled_ratchets"));
        assert!(UPGRADE_NOTICE.contains("disabled_ratchets"));
    }
}
