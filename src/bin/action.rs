use std::{env::set_current_dir, path::PathBuf};

use anyhow::Context;
use clap::{Parser, Subcommand};
use clap_verbosity_flag::{InfoLevel, Verbosity, log::LevelFilter};
use hoomd_workflow::simulate_one;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[command(flatten)]
    verbose: Verbosity<InfoLevel>,    
}

#[derive(Subcommand)]
enum Commands {
    Simulate { directory: PathBuf },
}

fn main() -> anyhow::Result<()> {
    let options = Cli::parse();

    let log_level = match options.verbose.log_level_filter() {
        LevelFilter::Off => "off",
        LevelFilter::Error => "error",
        LevelFilter::Warn => "warn",

        LevelFilter::Info => "info",
        LevelFilter::Debug => "debug",
        LevelFilter::Trace => "trace",
    };

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level))
        .format_timestamp(None)
        .init();
    
    set_current_dir("workspace").context("error switching to directory `workspace`")?;

    match &options.command {
        Commands::Simulate { directory } => {
            simulate_one(directory)?
        }
    }

    Ok(())
}
