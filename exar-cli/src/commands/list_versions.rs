use crate::OutputFormat;
use anyhow::{bail, Context, Result};
use exar::{database::db_connection, experiment::ExperimentVersion};
use itertools::Itertools;
use tabled::{builder::Builder, settings::Style};

/// For convenience, we can call `lsv` with either the full name of an experiment, or the ID of the experiment, which is
/// just an ascending number printed by `lse`. This function tries to figure out which of the two options it is and returns
/// the correct versions of the experiment
fn fetch_experiment_versions(experiment_name_or_id: &str) -> Result<Vec<ExperimentVersion>> {
    let db = db_connection();
    let mut versions = db
        .fetch_all_experiment_versions_by_name(experiment_name_or_id)
        .context("Failed to fetch experiment versions from database")?;
    if versions.is_empty() {
        let maybe_index = experiment_name_or_id.parse::<usize>().with_context(|| {
            format!("No experiment with name or ID {experiment_name_or_id} found")
        })? - 1;
        let mut experiments = db
            .fetch_experiments()
            .context("Failed to fetch experiments from database")?;
        experiments.sort();

        if maybe_index >= experiments.len() {
            bail!("Invalid experiment name or ID {experiment_name_or_id}");
        }
        let name = &experiments[maybe_index];
        versions = db
            .fetch_all_experiment_versions_by_name(&name)
            .context("Failed to fetch experiment versions from database")?;
    }
    Ok(versions)
}

pub fn list_versions(experiment_name_or_id: &str, output_format: OutputFormat) -> Result<()> {
    let mut versions = fetch_experiment_versions(experiment_name_or_id)?;
    // Sort descending by date
    versions.sort_by(|a, b| b.date().cmp(&a.date()));

    match output_format {
        OutputFormat::Table => print_versions_as_table(&versions),
        _ => unimplemented!(),
        // OutputFormat::CSV => print_experiments_as_csv(&experiment_names),
        // OutputFormat::JSON => print_experiments_as_json(&experiment_names),
        // OutputFormat::YAML => print_experiments_as_yaml(&experiment_names),
    }
}

fn print_versions_as_table(versions: &[ExperimentVersion]) -> Result<()> {
    let mut table_builder = Builder::default();
    table_builder.set_header([
        "ID",
        "Date",
        "Description",
        "Researchers",
        "Input variables",
        "Output variables",
    ]);

    for version in versions {
        table_builder.push_record([
            version.id().to_string(),
            version.date().to_string(),
            version.description().to_string(),
            version.researchers().iter().join(";"),
            version
                .input_variables()
                .iter()
                .map(|v| v.to_string())
                .join(";"),
            version
                .output_variables()
                .iter()
                .map(|v| v.to_string())
                .join(";"),
        ]);
    }

    let mut table = table_builder.build();
    table.with(Style::modern());

    println!("{table}");
    Ok(())
}
