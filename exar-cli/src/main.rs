mod commands;
mod configuration;
pub mod generic_table;
pub mod statistics;
pub mod util;

use std::fmt::Display;

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use commands::{
    configure::configure, init::init, list_experiments::list_experiments,
    list_instances::list_instances, list_runs::list_runs, list_versions::list_versions,
};

use crate::{commands::configure::initial_configuration, configuration::Configuration};

#[derive(Parser)]
#[command(name = "exar CLI")]
#[command(author = "Pascal Bormann <pascal@pascalbormann.de>")]
#[command(version)]
#[command(about = "CLI for the exar crate", long_about = None)]
struct Args {
    /// Which configuration to use? If omitted, the default configuration is used
    config: Option<String>,
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
    /// Configure the database connections for this tool
    Configure,
    /// Initialize a database so that it can store data in the `exar` format
    Init,
    /// List all known experiments in the database of the current configuration
    #[command(name = "lse")]
    ListExperiments {
        /// Format in which the list is printed
        #[arg(short, long, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    /// List all versions of the given experiment that are stored in the database of the current configuration
    #[command(name = "lsv")]
    ListVersions {
        /// Name of the experiment, or index as shown by `lse`
        name_or_id: String,
        /// Format in which the list is printed
        #[arg(short, long, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
    /// Lists all instances of the given experiment version that are stored in the database of the current configuration
    #[command(name = "lsi")]
    ListInstances {
        /// ID of the version to list all instances for, as shown by `lsv`, or name of the experiment if the `--latest` flag
        /// is set
        version_id_or_name: String,
        /// Format in which the list is printed
        #[arg(short, long, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
        /// Prints a statistical overview of the runs for each instance
        #[arg(short, long, default_value_t = false)]
        statistics: bool,
        /// List instances for the latest version of the experiment
        #[arg(short, long, default_value_t = false)]
        latest: bool,
    },
    /// List all runs of the given experiment instance that are stored in the database of the current configuration
    #[command(name = "lsr")]
    ListRuns {
        /// ID of the instance to list all runs for, as shown by `lsi`
        instance_id: String,
        /// Format in which the list is printed
        #[arg(short, long, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
        /// Instead of printing each individual run, print a statistical overview of all runs by combining measurements
        #[arg(short, long, default_value_t = false)]
        statistics: bool,
        // TODO Time range filtering
    },
}

fn load_config(config_name: Option<&String>) -> Result<()> {
    let configuration = Configuration::load()?;
    if let Some(config) = configuration {
        if let Some(config_name) = config_name {
            let matching_config = config
                .get_config_by_name(&config_name)
                .ok_or_else(|| anyhow!("No config with name {config_name} found"))?;
            matching_config.apply();
        } else {
            config.apply_default_config();
        }
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
    let args = Args::parse();
    load_config(args.config.as_ref()).context("Failed to load default configuration")?;

    match args.command {
        Commands::Configure => configure(),
        Commands::Init => init(),
        Commands::ListExperiments { format } => list_experiments(format),
        Commands::ListVersions { name_or_id, format } => list_versions(&name_or_id, format),
        Commands::ListInstances {
            version_id_or_name,
            format,
            statistics,
            latest,
        } => list_instances(&version_id_or_name, format, statistics, latest),
        Commands::ListRuns {
            instance_id,
            format,
            statistics,
        } => list_runs(&instance_id, format, statistics),
    }
}
