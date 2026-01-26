//! Ratchet CLI entry point

use clap::Parser;
use ratchet::cli::{Command, args::Cli};
use std::process;

fn main() {
    let cli = Cli::parse();

    let exit_code = match cli.command {
        Command::Init { force } => match ratchet::cli::init::run_init(force) {
            Ok(_) => {
                println!("Created ratchet.toml. Uncomment your languages to start checking.");
                0
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                2
            }
        },
        Command::Check { paths, format } => ratchet::cli::check::run_check(&paths, format),
        Command::Bump {
            rule_id,
            region,
            count,
            all,
        } => ratchet::cli::bump::run_bump(rule_id.as_deref(), &region, count, all),
        Command::Tighten { rule_id, region } => {
            ratchet::cli::tighten::run_tighten(rule_id.as_deref(), region.as_deref())
        }
        Command::List { format } => ratchet::cli::list::run_list(format),
        Command::MergeDriver {
            base,
            current,
            other,
        } => ratchet::cli::merge_driver::run_merge_driver(&base, &current, &other),
    };

    process::exit(exit_code);
}
