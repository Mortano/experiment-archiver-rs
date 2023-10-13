use std::collections::HashSet;

use crate::{
    generic_table::GenericTable,
    util::{print_serializable_as_json, print_serializable_as_yaml},
    OutputFormat,
};
use anyhow::{bail, Context, Result};
use exar::{database::db_connection, experiment::ExperimentVersion};
use itertools::Itertools;
use serde::Serialize;

#[derive(Serialize)]
struct SerializableExperimentVersion {
    id: String,
    name: String,
    version: String,
    date: String,
    description: String,
    researchers: HashSet<String>,
    input_variables: Vec<String>,
    output_variables: Vec<String>,
}

impl From<&ExperimentVersion> for SerializableExperimentVersion {
    fn from(value: &ExperimentVersion) -> Self {
        Self {
            date: value.date().to_string(),
            description: value.description().to_string(),
            id: value.id().to_string(),
            input_variables: value
                .input_variables()
                .iter()
                .map(|v| v.to_string())
                .collect(),
            name: value.name().to_string(),
            output_variables: value
                .output_variables()
                .iter()
                .map(|v| v.to_string())
                .collect(),
            researchers: value.researchers().clone(),
            version: value.version().to_string(),
        }
    }
}

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
        OutputFormat::CSV => print_versions_as_csv(&versions),
        OutputFormat::JSON => {
            let serializable = to_serializables(&versions);
            print_serializable_as_json(&serializable)
        }
        OutputFormat::YAML => {
            let serializable = to_serializables(&versions);
            print_serializable_as_yaml(&serializable)
        }
    }
}

fn to_generic_table(versions: &[ExperimentVersion]) -> GenericTable {
    let header = vec![
        "ID".to_string(),
        "Date".to_string(),
        "Description".to_string(),
        "Researchers".to_string(),
        "Input variables".to_string(),
        "Output variables".to_string(),
    ];

    let mut rows = vec![];
    for version in versions {
        rows.push(vec![
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

    GenericTable::new(header, rows)
}

fn to_serializables(versions: &[ExperimentVersion]) -> Vec<SerializableExperimentVersion> {
    versions.iter().map(|e| e.into()).collect()
}

fn print_versions_as_table(versions: &[ExperimentVersion]) -> Result<()> {
    let table = to_generic_table(versions);
    table.write_pretty(std::io::stdout())
}

fn print_versions_as_csv(versions: &[ExperimentVersion]) -> Result<()> {
    let table = to_generic_table(versions);
    table.write_csv(std::io::stdout())
}
