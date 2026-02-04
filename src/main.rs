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
            Err(e) => {
                eprintln!("Error: {}", e);
                2
            }
        },
        Command::Check {
            paths,
            format,
            verbose,
        } => ratchets::cli::check::run_check(&paths, format, verbose),
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
