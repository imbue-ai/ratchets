//! CLI argument parsing using clap

use clap::{Parser, Subcommand, ValueEnum};

/// Output format for ratchet commands
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable output
    Human,
    /// JSON Lines format (one JSON object per line)
    Jsonl,
}

/// Color output choice
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ColorChoice {
    /// Automatically detect if terminal supports color
    Auto,
    /// Always use color
    Always,
    /// Never use color
    Never,
}

/// Ratchet CLI main entry point
#[derive(Parser, Debug)]
#[command(name = "ratchet")]
#[command(about = "Progressive lint enforcement for human and AI developers")]
#[command(version)]
pub struct Cli {
    /// Subcommand to execute
    #[command(subcommand)]
    pub command: Command,

    /// Output coloring
    #[arg(long, global = true, default_value = "auto")]
    pub color: ColorChoice,
}

/// Available ratchet subcommands
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Check that violations are within budget
    Check {
        /// Paths to check (defaults to current directory)
        #[arg(default_value = ".")]
        paths: Vec<String>,

        /// Output format
        #[arg(short, long, default_value = "human")]
        format: OutputFormat,
    },

    /// Initialize ratchet in this repository
    Init {
        /// Overwrite existing files
        #[arg(long)]
        force: bool,
    },

    /// Increase a rule's violation budget
    Bump {
        /// Rule ID to bump (optional when --all is used)
        #[arg(required_unless_present = "all")]
        rule_id: Option<String>,

        /// Region to bump (defaults to root)
        #[arg(long, default_value = ".")]
        region: String,

        /// New count (auto-detects if not specified)
        #[arg(long, conflicts_with = "all")]
        count: Option<u64>,

        /// Bump all rules to their current violation counts
        #[arg(long, conflicts_with = "region")]
        all: bool,
    },

    /// Reduce budgets to match current violations
    Tighten {
        /// Specific rule to tighten (tightens all if omitted)
        rule_id: Option<String>,

        /// Specific region to tighten
        #[arg(long)]
        region: Option<String>,
    },

    /// List all enabled rules
    List {
        /// Output format
        #[arg(short, long, default_value = "human")]
        format: OutputFormat,
    },

    /// Git merge driver for ratchet-counts.toml
    MergeDriver {
        /// Base version (ancestor)
        base: String,

        /// Current version (ours)
        current: String,

        /// Other version (theirs)
        other: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn test_verify_cli() {
        // Verify that the CLI struct is properly configured
        Cli::command().debug_assert();
    }

    #[test]
    fn test_check_default_args() {
        let cli = Cli::parse_from(["ratchet", "check"]);
        match cli.command {
            Command::Check { paths, format } => {
                assert_eq!(paths, vec!["."]);
                assert_eq!(format, OutputFormat::Human);
            }
            _ => panic!("Expected Check command"),
        }
        assert_eq!(cli.color, ColorChoice::Auto);
    }

    #[test]
    fn test_check_with_paths() {
        let cli = Cli::parse_from(["ratchet", "check", "src/", "tests/"]);
        match cli.command {
            Command::Check { paths, .. } => {
                assert_eq!(paths, vec!["src/", "tests/"]);
            }
            _ => panic!("Expected Check command"),
        }
    }

    #[test]
    fn test_check_with_format() {
        let cli = Cli::parse_from(["ratchet", "check", "--format", "jsonl"]);
        match cli.command {
            Command::Check { format, .. } => {
                assert_eq!(format, OutputFormat::Jsonl);
            }
            _ => panic!("Expected Check command"),
        }
    }

    #[test]
    fn test_check_short_format() {
        let cli = Cli::parse_from(["ratchet", "check", "-f", "jsonl"]);
        match cli.command {
            Command::Check { format, .. } => {
                assert_eq!(format, OutputFormat::Jsonl);
            }
            _ => panic!("Expected Check command"),
        }
    }

    #[test]
    fn test_init_default() {
        let cli = Cli::parse_from(["ratchet", "init"]);
        match cli.command {
            Command::Init { force } => {
                assert!(!force);
            }
            _ => panic!("Expected Init command"),
        }
    }

    #[test]
    fn test_init_with_force() {
        let cli = Cli::parse_from(["ratchet", "init", "--force"]);
        match cli.command {
            Command::Init { force } => {
                assert!(force);
            }
            _ => panic!("Expected Init command"),
        }
    }

    #[test]
    fn test_bump_minimal() {
        let cli = Cli::parse_from(["ratchet", "bump", "no-unwrap"]);
        match cli.command {
            Command::Bump {
                rule_id,
                region,
                count,
                all,
            } => {
                assert_eq!(rule_id, Some("no-unwrap".to_string()));
                assert_eq!(region, ".");
                assert_eq!(count, None);
                assert!(!all);
            }
            _ => panic!("Expected Bump command"),
        }
    }

    #[test]
    fn test_bump_with_region() {
        let cli = Cli::parse_from(["ratchet", "bump", "no-unwrap", "--region", "src/legacy"]);
        match cli.command {
            Command::Bump {
                rule_id,
                region,
                count,
                all,
            } => {
                assert_eq!(rule_id, Some("no-unwrap".to_string()));
                assert_eq!(region, "src/legacy");
                assert_eq!(count, None);
                assert!(!all);
            }
            _ => panic!("Expected Bump command"),
        }
    }

    #[test]
    fn test_bump_with_count() {
        let cli = Cli::parse_from(["ratchet", "bump", "no-unwrap", "--count", "20"]);
        match cli.command {
            Command::Bump {
                rule_id,
                region,
                count,
                all,
            } => {
                assert_eq!(rule_id, Some("no-unwrap".to_string()));
                assert_eq!(region, ".");
                assert_eq!(count, Some(20));
                assert!(!all);
            }
            _ => panic!("Expected Bump command"),
        }
    }

    #[test]
    fn test_bump_full() {
        let cli = Cli::parse_from([
            "ratchet",
            "bump",
            "no-unwrap",
            "--region",
            "src/legacy",
            "--count",
            "20",
        ]);
        match cli.command {
            Command::Bump {
                rule_id,
                region,
                count,
                all,
            } => {
                assert_eq!(rule_id, Some("no-unwrap".to_string()));
                assert_eq!(region, "src/legacy");
                assert_eq!(count, Some(20));
                assert!(!all);
            }
            _ => panic!("Expected Bump command"),
        }
    }

    #[test]
    fn test_tighten_all() {
        let cli = Cli::parse_from(["ratchet", "tighten"]);
        match cli.command {
            Command::Tighten { rule_id, region } => {
                assert_eq!(rule_id, None);
                assert_eq!(region, None);
            }
            _ => panic!("Expected Tighten command"),
        }
    }

    #[test]
    fn test_tighten_specific_rule() {
        let cli = Cli::parse_from(["ratchet", "tighten", "no-unwrap"]);
        match cli.command {
            Command::Tighten { rule_id, region } => {
                assert_eq!(rule_id, Some("no-unwrap".to_string()));
                assert_eq!(region, None);
            }
            _ => panic!("Expected Tighten command"),
        }
    }

    #[test]
    fn test_tighten_with_region() {
        let cli = Cli::parse_from(["ratchet", "tighten", "--region", "src/"]);
        match cli.command {
            Command::Tighten { rule_id, region } => {
                assert_eq!(rule_id, None);
                assert_eq!(region, Some("src/".to_string()));
            }
            _ => panic!("Expected Tighten command"),
        }
    }

    #[test]
    fn test_tighten_rule_and_region() {
        let cli = Cli::parse_from(["ratchet", "tighten", "no-unwrap", "--region", "src/"]);
        match cli.command {
            Command::Tighten { rule_id, region } => {
                assert_eq!(rule_id, Some("no-unwrap".to_string()));
                assert_eq!(region, Some("src/".to_string()));
            }
            _ => panic!("Expected Tighten command"),
        }
    }

    #[test]
    fn test_list_default() {
        let cli = Cli::parse_from(["ratchet", "list"]);
        match cli.command {
            Command::List { format } => {
                assert_eq!(format, OutputFormat::Human);
            }
            _ => panic!("Expected List command"),
        }
    }

    #[test]
    fn test_list_with_format() {
        let cli = Cli::parse_from(["ratchet", "list", "--format", "jsonl"]);
        match cli.command {
            Command::List { format } => {
                assert_eq!(format, OutputFormat::Jsonl);
            }
            _ => panic!("Expected List command"),
        }
    }

    #[test]
    fn test_list_short_format() {
        let cli = Cli::parse_from(["ratchet", "list", "-f", "jsonl"]);
        match cli.command {
            Command::List { format } => {
                assert_eq!(format, OutputFormat::Jsonl);
            }
            _ => panic!("Expected List command"),
        }
    }

    #[test]
    fn test_merge_driver() {
        let cli = Cli::parse_from([
            "ratchet",
            "merge-driver",
            "base.toml",
            "current.toml",
            "other.toml",
        ]);
        match cli.command {
            Command::MergeDriver {
                base,
                current,
                other,
            } => {
                assert_eq!(base, "base.toml");
                assert_eq!(current, "current.toml");
                assert_eq!(other, "other.toml");
            }
            _ => panic!("Expected MergeDriver command"),
        }
    }

    #[test]
    fn test_global_color_flag() {
        let cli = Cli::parse_from(["ratchet", "--color", "always", "check"]);
        assert_eq!(cli.color, ColorChoice::Always);

        let cli = Cli::parse_from(["ratchet", "--color", "never", "list"]);
        assert_eq!(cli.color, ColorChoice::Never);

        let cli = Cli::parse_from(["ratchet", "--color", "auto", "init"]);
        assert_eq!(cli.color, ColorChoice::Auto);
    }

    #[test]
    fn test_color_flag_before_subcommand() {
        let cli = Cli::parse_from(["ratchet", "--color", "always", "check", "src/"]);
        assert_eq!(cli.color, ColorChoice::Always);
        match cli.command {
            Command::Check { paths, .. } => {
                assert_eq!(paths, vec!["src/"]);
            }
            _ => panic!("Expected Check command"),
        }
    }

    #[test]
    fn test_help_contains_about() {
        let help = Cli::command().render_help().to_string();
        assert!(help.contains("Progressive lint enforcement"));
    }

    #[test]
    fn test_version_flag() {
        // Just verify that --version doesn't panic
        let result = Cli::try_parse_from(["ratchet", "--version"]);
        // This will fail with DisplayVersion error, which is expected
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_format() {
        let result = Cli::try_parse_from(["ratchet", "check", "--format", "invalid"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_color() {
        let result = Cli::try_parse_from(["ratchet", "--color", "invalid", "check"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_required_args() {
        // Bump requires rule_id when --all is not used
        let result = Cli::try_parse_from(["ratchet", "bump"]);
        assert!(result.is_err());

        // MergeDriver requires three positional args
        let result = Cli::try_parse_from(["ratchet", "merge-driver", "base"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_bump_all_flag() {
        let cli = Cli::parse_from(["ratchet", "bump", "--all"]);
        match cli.command {
            Command::Bump {
                rule_id,
                region,
                count,
                all,
            } => {
                assert_eq!(rule_id, None);
                assert_eq!(region, ".");
                assert_eq!(count, None);
                assert!(all);
            }
            _ => panic!("Expected Bump command"),
        }
    }

    #[test]
    fn test_bump_all_with_rule_id() {
        // Using both rule_id and --all should work (rule_id is just ignored)
        let cli = Cli::parse_from(["ratchet", "bump", "no-unwrap", "--all"]);
        match cli.command {
            Command::Bump {
                rule_id,
                region,
                count,
                all,
            } => {
                assert_eq!(rule_id, Some("no-unwrap".to_string()));
                assert_eq!(region, ".");
                assert_eq!(count, None);
                assert!(all);
            }
            _ => panic!("Expected Bump command"),
        }
    }

    #[test]
    fn test_bump_all_conflicts_with_region() {
        // --all conflicts with --region
        let result = Cli::try_parse_from(["ratchet", "bump", "--all", "--region", "src/"]);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("--all") || err_msg.contains("--region"));
    }

    #[test]
    fn test_bump_all_conflicts_with_count() {
        // --all conflicts with --count
        let result = Cli::try_parse_from(["ratchet", "bump", "--all", "--count", "20"]);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("--all") || err_msg.contains("--count"));
    }
}
