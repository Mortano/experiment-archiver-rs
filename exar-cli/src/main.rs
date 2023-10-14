mod commands;
mod configuration;
pub mod generic_table;
pub mod statistics;
pub mod util;

use std::fmt::Display;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use commands::{
    configure::configure, list_experiments::list_experiments, list_instances::list_instances,
    list_runs::list_runs, list_versions::list_versions,
};

use crate::{commands::configure::initial_configuration, configuration::Configuration};

#[derive(Parser)]
#[command(name = "exar CLI")]
#[command(author = "Pascal Bormann <pascal@pascalbormann.de>")]
#[command(version)]
#[command(about = "CLI for the exar crate", long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

/// Output format for many of the subcommands that print data
#[derive(ValueEnum, Copy, Clone, Debug, PartialEq, Eq)]
pub enum OutputFormat {
    /// A pretty table
    Table,
    /// CSV with comma as separator
    CSV,
    /// JSON
    JSON,
    /// YAML
    YAML,
}

impl Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.to_possible_value()
            .expect("no values are skipped")
            .get_name()
            .fmt(f)
    }
}

#[derive(Subcommand)]
enum Commands {
    Configure,
    #[command(name = "lse")]
    ListExperiments {
        #[arg(short, long, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    #[command(name = "lsv")]
    ListVersions {
        /// Name of the experiment, or index as shown by `lse`
        name_or_id: String,
        #[arg(short, long, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    #[command(name = "lsi")]
    ListInstances {
        /// ID of the version to list all instances for, as shown by `lsv`
        version_id: String,
        #[arg(short, long, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
        /// Prints a statistical overview of the runs for each instance
        #[arg(short, long, default_value_t = false)]
        statistics: bool,
    },
    #[command(name = "lsr")]
    ListRuns {
        /// ID of the instance to list all runs for, as shown by `lsi`
        instance_id: String,
        #[arg(short, long, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
        /// Instead of printing each individual run, print a statistical overview of all runs by combining numerical measurements
        #[arg(short, long, default_value_t = false)]
        statistics: bool,
        // TODO Time range filtering
    }, // ListRuns {
       //     experiment_name: String,
       //     #[arg(short, long, default_value_t = false)]
       //     as_csv: bool,
       // },
       // PrintRun {
       //     run_id: String,
       //     #[arg(short, long, default_value_t = false)]
       //     as_csv: bool,
       // },
       // PrintAllRuns {
       //     experiment_name: String,
       //     #[arg(short, long, default_value_t = false)]
       //     as_csv: bool,
       // },
       // DeleteExperiment {
       //     experiment_name: String,
       // },
       // DeleteRuns {
       //     experiment_name: String,
       //     #[arg(
       //         help = "The ID(s) of all runs that should be deleted. Can be a single number (e.g. \"4\") to delete one run, a comma-separated list (e.g. \"1,2,4\") to delete multiple runs, or an inclusive range (e.g. \"1-6\") to delete a range of consecutive runs"
       //     )]
       //     run_numbers: String,
       // },
}

fn load_config() -> Result<()> {
    let configuration = Configuration::load()?;
    if let Some(config) = configuration {
        config.apply_default_config();
    } else {
        let (name, new_default_config) = initial_configuration()?;
        new_default_config.apply();

        let mut configuration = Configuration::default();
        configuration.add_entry(name, new_default_config);
        configuration
            .store()
            .context("Failed to save configuration")?;
    }
    Ok(())
}

fn main() -> Result<()> {
    load_config().context("Failed to load default configuration")?;

    let args = Args::parse();
    match args.command {
        Commands::Configure => configure(),
        Commands::ListExperiments { format } => list_experiments(format),
        Commands::ListVersions { name_or_id, format } => list_versions(&name_or_id, format),
        Commands::ListInstances {
            version_id,
            format,
            statistics,
        } => list_instances(&version_id, format, statistics),
        Commands::ListRuns {
            instance_id,
            format,
            statistics,
        } => list_runs(&instance_id, format, statistics),
    }
}
