use std::{io::Write, path::PathBuf, time::SystemTime};

use anyhow::{anyhow, bail, Context, Result};
use chrono::{Local, NaiveDateTime};
use clap::{Parser, Subcommand};
use experiment_archiver::Experiment;
use serde::{Deserialize, Serialize};
use tabled::{builder::Builder, Table, Tabled};

#[derive(Parser)]
#[command(name = "Experiment Archive CLI")]
#[command(author = "Pascal Bormann <pascal@pascalbormann.de>")]
#[command(version)]
#[command(about = "CLI for the experiment-archive crate", long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Configure {},
    ListExperiments {},
    ListRuns { experiment_name: String },
    PrintRun { run_id: String },
    PrintAllRuns { experiment_name: String },
}

#[derive(Serialize, Deserialize)]
struct Configuration {
    user: String,
    password: String,
    host: String,
    port: String,
    database_name: String,
}

impl Configuration {
    /// Applies this configuration. This sets the required environment variables for the experiment-archive crate
    pub fn apply(&self) {
        std::env::set_var("PSQL_USER", &self.user);
        std::env::set_var("PSQL_PWD", &self.password);
        std::env::set_var("PSQL_HOST", &self.host);
        std::env::set_var("PSQL_PORT", &self.port);
        std::env::set_var("PSQL_DBNAME", &self.database_name);
    }

    /// Store this configuration to disk
    pub fn store(&self) -> Result<()> {
        let path = Self::default_path()?;
        let config_json = serde_json::to_string_pretty(self)
            .context("Failed to convert configuration to JSON format")?;
        std::fs::write(path, config_json).context("Failed to write configuration file")
    }

    /// Returns the default file path of the config file
    pub fn default_path() -> Result<PathBuf> {
        let user_config_dir = dirs::config_dir()
            .context("Could not determine path of current user's configuration directory")?;
        let config_file_path = user_config_dir
            .join("experiment_archive_cli")
            .join("cli.config");
        Ok(config_file_path)
    }
}

/// Tries to load the current config file. If the file does not exist, Ok(false) is returned,
/// if any error occurs while trying to read the config file, Err is returned
fn load_config() -> Result<bool> {
    let config_file_path = Configuration::default_path()?;
    let config_file_parent_dir = config_file_path
        .parent()
        .expect("Could not get parent directory of default config path");
    std::fs::create_dir_all(&config_file_parent_dir)
        .context("Could not create configuration directory")?;

    if !config_file_path.exists() {
        Ok(false)
    } else {
        match serde_json::from_str::<Configuration>(
            &std::fs::read_to_string(&config_file_path).context("Could not load config file")?,
        ) {
            Ok(parsed_config) => {
                parsed_config.apply();
                Ok(true)
            }
            Err(why) => {
                eprintln!("Configuration file contains invalid content and could not be parsed ({why}). Removing malformed config file...");
                Ok(false)
            }
        }
    }
}

/// Run tool configuration
fn configure() -> Result<()> {
    let mut host = String::default();
    print!("Enter hostname of postgres database: ");
    std::io::stdout().flush()?;
    std::io::stdin()
        .read_line(&mut host)
        .context("Failed to read line")?;

    let mut port = String::default();
    print!("Enter port of postgres database: ");
    std::io::stdout().flush()?;
    std::io::stdin()
        .read_line(&mut port)
        .context("Failed to read line")?;

    let mut user = String::default();
    print!("Enter username of postgres database: ");
    std::io::stdout().flush()?;
    std::io::stdin()
        .read_line(&mut user)
        .context("Failed to read line")?;

    let mut password = String::default();
    print!("Enter password: ");
    std::io::stdout().flush()?;
    std::io::stdin()
        .read_line(&mut password)
        .context("Failed to read line")?;

    let mut database_name = String::default();
    print!("Enter name of database: ");
    std::io::stdout().flush()?;
    std::io::stdin()
        .read_line(&mut database_name)
        .context("Failed to read line")?;

    let config = Configuration {
        database_name: database_name.trim().into(),
        user: user.trim().into(),
        password: password.trim().into(),
        host: host.trim().into(),
        port: port.trim().into(),
    };
    config
        .store()
        .context("Failed to store new configuration to disk")?;
    config.apply();
    Ok(())
}

fn list_experiments() -> Result<()> {
    let all_experiments = Experiment::all().context("Error while fetching experiments")?;

    #[derive(Tabled)]
    struct TabledExperiment {
        name: String,
        researcher: String,
        description_short: String,
        variable_names: String,
    }

    const MAX_DESCRIPTION_LENGTH: usize = 32;

    let table = Table::new(all_experiments.iter().map(|ex| {
        let description_short = if ex.description().len() > MAX_DESCRIPTION_LENGTH {
            format!("{}...", &ex.description()[..MAX_DESCRIPTION_LENGTH])
        } else {
            ex.description().to_owned()
        };
        let variable_names = ex
            .variables()
            .map(|var| var.template().name())
            .collect::<Vec<_>>()
            .join(",");
        TabledExperiment {
            description_short,
            name: ex.name().to_owned(),
            researcher: ex.researcher().to_owned(),
            variable_names,
        }
    }));

    print!("{table}");

    Ok(())
}

fn list_runs(experiment_name: &str) -> Result<()> {
    let experiment = Experiment::from_name(experiment_name)
        .context("Failed to query database for experiments")?;
    match experiment {
        None => bail!("No experiment with name {experiment_name} found"),
        Some(experiment) => {
            let all_runs = experiment
                .all_runs()
                .context("Failed to get runs for experiment")?;

            #[derive(Tabled)]
            struct TabledRun {
                run_number: usize,
                run_id: String,
                timestamp: String,
            }

            let table = Table::new(all_runs.iter().map(|run| {
                let timestamp = run
                    .measurements()
                    .first()
                    .map(|measurement| {
                        let time_since_epoch = measurement
                            .timestamp()
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .expect("Failed to get time since epoch");

                        NaiveDateTime::from_timestamp_millis(time_since_epoch.as_millis() as i64)
                            .map(|date_time| date_time.and_utc().with_timezone(&Local).to_string())
                            .unwrap_or("unknown".into())
                    })
                    .unwrap_or("unknown".into());
                TabledRun {
                    run_number: run.run_number(),
                    run_id: run.id().to_owned(),
                    timestamp,
                }
            }));

            print!("{table}");
        }
    }
    Ok(())
}

fn print_run(run_id: &str) -> Result<()> {
    let experiment = Experiment::from_run_id(run_id)
        .context("Failed to fetch experiment for run ID")?
        .ok_or(anyhow!("No experiment found for run ID"))?;
    let run = experiment
        .run_from_id(run_id)
        .context("Failed to fetch run from DB")?
        .ok_or(anyhow!("No run found for run ID"))?;

    let mut table_builder = Builder::default();
    let header = std::iter::once(String::from("run_number")).chain(
        run.measurements()
            .iter()
            .map(|measurement| measurement.variable().template().name().to_owned()),
    );
    table_builder.set_header(header);

    let row = std::iter::once(run.run_number().to_string()).chain(
        run.measurements()
            .iter()
            .map(|measurement| measurement.value().to_owned()),
    );
    table_builder.push_record(row);

    let table = table_builder.build();
    print!("{table}");

    Ok(())
}

fn print_all_runs(experiment_name: &str) -> Result<()> {
    let experiment = Experiment::from_name(experiment_name)
        .context("Failed to query database for experiments")?;
    match experiment {
        None => bail!("No experiment with name {experiment_name} found"),
        Some(experiment) => {
            let all_runs = experiment
                .all_runs()
                .context("Failed to get runs for experiment")?;

            let expected_variables = experiment.variables().collect::<Vec<_>>();

            let mut table_builder = Builder::default();
            let header = std::iter::once(String::from("run_number")).chain(
                expected_variables
                    .iter()
                    .map(|variable| variable.template().name().to_owned()),
            );
            table_builder.set_header(header);

            for run in all_runs {
                let row = std::iter::once(run.run_number().to_string()).chain(
                    expected_variables.iter().map(|variable| {
                        run.measurements()
                            .iter()
                            .find(|measurement| measurement.variable() == *variable)
                            .map(|measurement| measurement.value().to_owned())
                            .unwrap_or("N/A".to_owned())
                    }),
                );
                table_builder.push_record(row);
            }

            let table = table_builder.build();
            print!("{table}");
        }
    }

    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();

    if load_config().context("Could not load configuration")? == false {
        configure().context("Error while configuring default parameters")?;
    }

    match &args.command {
        Commands::Configure {} => {
            configure().context("Error while configuring default parameters")?
        }
        Commands::ListExperiments {} => list_experiments().context("Failed to list experiments")?,
        Commands::ListRuns { experiment_name } => {
            list_runs(&experiment_name).context("Failed to list runs for experiment")?
        }
        Commands::PrintRun { run_id } => print_run(&run_id).context("Failed to print run")?,
        Commands::PrintAllRuns { experiment_name } => {
            print_all_runs(experiment_name).context("Failed to print all runs of experiment")?
        }
    }

    Ok(())
}
