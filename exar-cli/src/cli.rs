use std::{io::Write, path::PathBuf, time::SystemTime};

use anyhow::{anyhow, bail, Context, Result};
use chrono::{Local, NaiveDateTime};
use clap::{Parser, Subcommand};
use experiment_archiver::Experiment;
use serde::{Deserialize, Serialize};
use tabled::{
    builder::Builder,
    settings::{object::Rows, Modify, Style, Width},
};

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
    ListExperiments {
        #[arg(short, long, default_value_t = false)]
        as_csv: bool,
    },
    ListRuns {
        experiment_name: String,
        #[arg(short, long, default_value_t = false)]
        as_csv: bool,
    },
    PrintRun {
        run_id: String,
        #[arg(short, long, default_value_t = false)]
        as_csv: bool,
    },
    PrintAllRuns {
        experiment_name: String,
        #[arg(short, long, default_value_t = false)]
        as_csv: bool,
    },
    DeleteExperiment {
        experiment_name: String,
    },
    DeleteRuns {
        experiment_name: String,
        #[arg(
            help = "The ID(s) of all runs that should be deleted. Can be a single number (e.g. \"4\") to delete one run, a comma-separated list (e.g. \"1,2,4\") to delete multiple runs, or an inclusive range (e.g. \"1-6\") to delete a range of consecutive runs"
        )]
        run_numbers: String,
    },
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

struct GenericTable {
    header: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl GenericTable {
    fn write_pretty<W: Write>(&self, mut writer: W) -> Result<()> {
        let mut table_builder = Builder::default();
        table_builder.set_header(&self.header);

        for row in &self.rows {
            table_builder.push_record(row);
        }

        let mut table = table_builder.build();

        // let (terminal_width, _) =
        //     termion::terminal_size().context("Can't determine terminal size")?;
        // table.with(Width::wrap(terminal_width as usize));
        table.with(Modify::new(Rows::new(..)).with(Width::wrap(24)));
        table.with(Style::modern());

        write!(writer, "{table}")?;

        Ok(())
    }

    fn write_csv<W: Write>(&self, mut writer: W) -> Result<()> {
        let cleanup_str_for_csv = |s: &String| -> String {
            let must_be_quoted = s.contains(&['\n', '\r', ',']);
            if !must_be_quoted {
                s.to_owned()
            } else {
                format!("\"{s}\"")
            }
        };

        let header = self
            .header
            .iter()
            .map(cleanup_str_for_csv)
            .collect::<Vec<_>>()
            .join(",");
        writeln!(writer, "{header}")?;
        for (idx, row) in self.rows.iter().enumerate() {
            let row = row
                .iter()
                .map(cleanup_str_for_csv)
                .collect::<Vec<_>>()
                .join(",");
            if idx == self.rows.len() - 1 {
                write!(writer, "{row}")?;
            } else {
                writeln!(writer, "{row}")?
            }
        }
        Ok(())
    }
}

fn list_experiments(as_csv: bool) -> Result<()> {
    let all_experiments = Experiment::all().context("Error while fetching experiments")?;

    const MAX_DESCRIPTION_LENGTH: usize = 32;

    let header = vec![
        "name".into(),
        "researcher".into(),
        "description".into(),
        "variable_names".into(),
    ];

    let rows = all_experiments
        .iter()
        .map(|ex| {
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

            vec![
                ex.name().to_owned(),
                ex.researcher().to_owned(),
                description_short,
                variable_names,
            ]
        })
        .collect();

    let generic_table = GenericTable { header, rows };

    if as_csv {
        generic_table.write_csv(std::io::stdout())?;
    } else {
        generic_table.write_pretty(std::io::stdout())?;
    }

    Ok(())
}

fn list_runs(experiment_name: &str, as_csv: bool) -> Result<()> {
    let experiment = Experiment::from_name(experiment_name)
        .context("Failed to query database for experiments")?;
    match experiment {
        None => bail!("No experiment with name {experiment_name} found"),
        Some(experiment) => {
            let all_runs = experiment
                .all_runs()
                .context("Failed to get runs for experiment")?;

            let header = vec![
                "run_number".to_owned(),
                "run_id".to_owned(),
                "timestamp".to_owned(),
            ];

            let rows = all_runs
                .iter()
                .map(|run| {
                    let timestamp = run
                        .measurements()
                        .first()
                        .map(|measurement| {
                            let time_since_epoch = measurement
                                .timestamp()
                                .duration_since(SystemTime::UNIX_EPOCH)
                                .expect("Failed to get time since epoch");

                            NaiveDateTime::from_timestamp_millis(
                                    time_since_epoch.as_millis() as i64
                                )
                                .map(|date_time| {
                                    date_time.and_utc().with_timezone(&Local).to_string()
                                })
                                .unwrap_or("unknown".into())
                        })
                        .unwrap_or("unknown".into());
                    vec![run.run_number().to_string(), run.id().to_owned(), timestamp]
                })
                .collect();

            let generic_table = GenericTable { header, rows };
            if as_csv {
                generic_table.write_csv(std::io::stdout())?;
            } else {
                generic_table.write_pretty(std::io::stdout())?;
            }
        }
    }
    Ok(())
}

/// Format a variable value for printing. This removes newlines and carriage returns from the string
/// so that it can be written as a single line of a table or CSV file
fn format_variable_value(value: &str) -> String {
    value.replace("\n", "").replace("\r", "")
}

fn print_run(run_id: &str, as_csv: bool) -> Result<()> {
    let experiment = Experiment::from_run_id(run_id)
        .context("Failed to fetch experiment for run ID")?
        .ok_or(anyhow!("No experiment found for run ID"))?;
    let run = experiment
        .run_from_id(run_id)
        .context("Failed to fetch run from DB")?
        .ok_or(anyhow!("No run found for run ID"))?;

    let header = std::iter::once(String::from("run_number")).chain(
        run.measurements()
            .iter()
            .map(|measurement| measurement.variable().template().name().to_owned()),
    );

    let row = std::iter::once(run.run_number().to_string()).chain(
        run.measurements()
            .iter()
            .map(|measurement| format_variable_value(measurement.value())),
    );

    let generic_table = GenericTable {
        header: header.collect(),
        rows: vec![row.collect()],
    };

    if as_csv {
        generic_table.write_csv(std::io::stdout())?;
    } else {
        generic_table.write_pretty(std::io::stdout())?;
    }

    Ok(())
}

fn print_all_runs(experiment_name: &str, as_csv: bool) -> Result<()> {
    let experiment = Experiment::from_name(experiment_name)
        .context("Failed to query database for experiments")?;
    match experiment {
        None => bail!("No experiment with name {experiment_name} found"),
        Some(experiment) => {
            let all_runs = experiment
                .all_runs()
                .context("Failed to get runs for experiment")?;

            let mut expected_variables = experiment.variables().collect::<Vec<_>>();
            expected_variables.sort_by(|a, b| a.template().name().cmp(b.template().name()));

            let header = std::iter::once(String::from("run_number"))
                .chain(
                    expected_variables
                        .iter()
                        .map(|variable| variable.template().name().to_owned()),
                )
                .collect();

            let rows = all_runs
                .iter()
                .map(|run| {
                    std::iter::once(run.run_number().to_string())
                        .chain(expected_variables.iter().map(|variable| {
                            run.measurements()
                                .iter()
                                .find(|measurement| measurement.variable().id() == variable.id())
                                .map(|measurement| format_variable_value(measurement.value()))
                                .unwrap_or("N/A".to_owned())
                        }))
                        .collect()
                })
                .collect();

            let table = GenericTable { header, rows };
            if as_csv {
                table.write_csv(std::io::stdout())?;
            } else {
                table.write_pretty(std::io::stdout())?;
            }
        }
    }

    Ok(())
}

fn delete_experiment(experiment_name: &str) -> Result<()> {
    let experiment = Experiment::from_name(experiment_name)
        .context("Failed to query database for experiments")?
        .ok_or(anyhow!(
            "No experiment with name \"{experiment_name}\" found"
        ))?;

    println!("Are you sure you want to delete all data for experiment \"{experiment_name}\"? This operation is not reversible! (y/n)");
    let mut input = String::default();
    std::io::stdin().read_line(&mut input)?;
    if input.trim() != "y" {
        return Ok(());
    }

    experiment.delete_from_database()?;

    Ok(())
}

fn parse_run_numbers_to_vec(run_numbers: &str) -> Result<Vec<usize>> {
    // Either `run_numbers` is a single number, or a list of numbers (which contains at least one comma), or
    // a range of numbers (which contains exactly one dash)
    if run_numbers.contains(',') {
        let numbers = run_numbers.split(',');
        numbers
            .map(|number| {
                number
                    .parse::<usize>()
                    .context("Could not parse run number")
            })
            .collect()
    } else if run_numbers.contains('-') {
        let split = run_numbers.split('-').collect::<Vec<_>>();
        if split.len() != 2 {
            bail!("Could not parse run numbers as range. Expected \"first-last\" but got {run_numbers} instead");
        }
        let first = split[0]
            .parse::<usize>()
            .context("Could not parse start value of run numbers range")?;
        let last_inclusive = split[1]
            .parse::<usize>()
            .context("Could not parse end value of run numbers range")?;
        if last_inclusive < first {
            bail!("End value of run numbers range must be >= start value");
        }
        Ok((first..=last_inclusive).collect::<Vec<_>>())
    } else {
        let single_run = run_numbers
            .parse::<usize>()
            .context("Could not parse run number as unsigned integer")?;
        Ok(vec![single_run])
    }
}

fn delete_runs(experiment_name: &str, run_numbers: &str) -> Result<()> {
    let run_numbers_vec =
        parse_run_numbers_to_vec(run_numbers).context("Failed to parse run numbers")?;

    let experiment = Experiment::from_name(experiment_name)
        .context("Failed to query database for experiments")?
        .ok_or(anyhow!(
            "No experiment with name \"{experiment_name}\" found"
        ))?;

    println!("Are you sure you want to delete run(s) {run_numbers} for experiment \"{experiment_name}\"? This operation is not reversible! (y/n)");
    let mut input = String::default();
    std::io::stdin().read_line(&mut input)?;
    if input.trim() != "y" {
        return Ok(());
    }

    experiment
        .delete_runs_from_database(run_numbers_vec.into_iter())
        .context("Failed to delete runs")?;

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
        Commands::ListExperiments { as_csv } => {
            list_experiments(*as_csv).context("Failed to list experiments")?
        }
        Commands::ListRuns {
            experiment_name,
            as_csv,
        } => list_runs(&experiment_name, *as_csv).context("Failed to list runs for experiment")?,
        Commands::PrintRun { run_id, as_csv } => {
            print_run(&run_id, *as_csv).context("Failed to print run")?
        }
        Commands::PrintAllRuns {
            experiment_name,
            as_csv,
        } => print_all_runs(experiment_name, *as_csv)
            .context("Failed to print all runs of experiment")?,
        Commands::DeleteExperiment { experiment_name } => {
            delete_experiment(&experiment_name).context("Failed to delete experiment")?
        }
        Commands::DeleteRuns {
            experiment_name,
            run_numbers,
        } => delete_runs(&experiment_name, &run_numbers).context("Failed to delete runs")?,
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_run_numbers() {
        // Nothing
        assert!(parse_run_numbers_to_vec("").is_err());

        // Parse single numbers
        {
            let res = parse_run_numbers_to_vec("1");
            assert!(res.is_ok());
            assert_eq!(vec![1], res.unwrap());
        }
        {
            let res = parse_run_numbers_to_vec("123");
            assert!(res.is_ok());
            assert_eq!(vec![123], res.unwrap());
        }

        // Parse list of numbers
        {
            let res = parse_run_numbers_to_vec("1,3");
            assert!(res.is_ok());
            assert_eq!(vec![1, 3], res.unwrap());
        }
        {
            let res = parse_run_numbers_to_vec("5,3,173");
            assert!(res.is_ok());
            assert_eq!(vec![5, 3, 173], res.unwrap());
        }
        assert!(parse_run_numbers_to_vec("1,").is_err());
        assert!(parse_run_numbers_to_vec(",").is_err());

        // Parse range of numbers
        {
            let res = parse_run_numbers_to_vec("1-3");
            assert!(res.is_ok());
            assert_eq!(vec![1, 2, 3], res.unwrap());
        }
        {
            let res = parse_run_numbers_to_vec("1-1");
            assert!(res.is_ok());
            assert_eq!(vec![1], res.unwrap());
        }
        {
            let res = parse_run_numbers_to_vec("9-11");
            assert!(res.is_ok());
            assert_eq!(vec![9, 10, 11], res.unwrap());
        }
        assert!(parse_run_numbers_to_vec("1-").is_err());
        assert!(parse_run_numbers_to_vec("-").is_err());
    }
}
