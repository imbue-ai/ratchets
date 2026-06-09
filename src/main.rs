//! Ratchet CLI entry point

use clap::Parser;
use ratchets::cli::{Command, args::Cli};
use std::process;

fn main() {
    let cli = Cli::parse();

    let exit_code = match cli.command {
        Command::Init { force } => match ratchets::cli::init::run_init(force) {
            Ok(_) => {
                println!("Created ratchets.toml. Uncomment your languages to start checking.");
                0
            }
            Err(ratchets::cli::init::InitError::ExistingV1Config) => {
                // Render the embedded upgrade notice (the same one `check` /
                // `bump` / `tighten` / `list` emit) before the generic error
                // line so users see the migration guide rather than a one-liner.
                ratchets::cli::upgrade_notice::print_to_stderr();
                eprintln!(
                    "Error: ratchets.toml already exists with version = \"1\". Migrate to v2 (see the upgrade notice above) or re-run with --force to overwrite."
                );
                2
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                2
            }
        },
        Command::Check {
            paths,
            format,
            verbose,
            since,
        } => ratchets::cli::check::run_check(&paths, format, verbose, since.as_deref()),
        Command::Bump {
            rule_id,
            region,
            count,
            all,
        } => ratchets::cli::bump::run_bump(rule_id.as_deref(), &region, count, all),
        Command::Tighten { rule_id, region } => {
            ratchets::cli::tighten::run_tighten(rule_id.as_deref(), region.as_deref())
        }
        Command::List { format } => ratchets::cli::list::run_list(format),
        Command::MergeDriver {
            base,
            current,
            other,
        } => ratchets::cli::merge_driver::run_merge_driver(&base, &current, &other),
    };

    process::exit(exit_code);
}
