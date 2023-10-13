use crate::{
    generic_table::GenericTable,
    util::{print_serializable_as_json, print_serializable_as_yaml, SerializableVariableValue},
    OutputFormat,
};
use anyhow::{anyhow, bail, Context, Result};
use exar::{database::db_connection, run::ExperimentRun};
use itertools::Itertools;
use serde::Serialize;

#[derive(Debug, Serialize)]
struct SerializableRun {
    id: String,
    date: String,
    measurements: Vec<SerializableVariableValue>,
}

impl From<&ExperimentRun<'_>> for SerializableRun {
    fn from(value: &ExperimentRun<'_>) -> Self {
        Self {
            id: value.id().to_string(),
            date: value.date().to_string(),
            measurements: value.measurements().iter().map(|m| m.into()).collect(),
        }
    }
}

pub fn list_runs(instance_id: &str, output_format: OutputFormat) -> Result<()> {
    // TODO This is a bit more complicated than I would like...
    let db = db_connection();
    let version = db
        .fetch_experiment_version_from_instance_id(instance_id)
        .with_context(|| {
            format!("Failed to fetch matching experiment version for instance ID {instance_id}")
        })?
        .ok_or_else(|| anyhow!("No experiment version found for instance {instance_id}"))?;
    let instance = db
        .fetch_instance_from_id(instance_id, &version)
        .with_context(|| format!("Failed to fetch instance {instance_id}"))?
        .ok_or_else(|| anyhow!("Instance {instance_id} not found"))?;
    let runs = db
        .fetch_all_runs_of_instance(&instance)
        .with_context(|| format!("Failed to fetch runs for instance {instance_id}"))?;

    if runs.is_empty() {
        bail!(
            "No runs found for instance {instance_id} of experiment {} @ version {}",
            version.name(),
            version.version()
        );
    }

    match output_format {
        OutputFormat::Table => print_runs_as_table(&runs),
        OutputFormat::CSV => print_runs_as_csv(&runs),
        OutputFormat::JSON => print_runs_as_json(&runs),
        OutputFormat::YAML => print_runs_as_yaml(&runs),
    }
}

fn runs_to_table(runs: &[ExperimentRun<'_>]) -> GenericTable {
    let mut sorted_variables = runs[0]
        .measurements()
        .iter()
        .map(|m| m.variable())
        .collect_vec();
    sorted_variables.sort_by(|a, b| a.name().cmp(b.name()));

    let header = ["ID", "Date"]
        .into_iter()
        .chain(sorted_variables.iter().map(|v| v.name()))
        .map(ToString::to_string)
        .collect_vec();
    let rows = runs
        .iter()
        .map(|run| -> Vec<String> {
            [run.id().to_string(), run.date().to_string()]
                .into_iter()
                .chain(sorted_variables.iter().map(|v| {
                    let matching_measurement = run
                        .measurements()
                        .iter()
                        .find(|m| m.variable() == *v)
                        .unwrap();
                    matching_measurement.value().to_string()
                }))
                .collect_vec()
        })
        .collect_vec();

    GenericTable::new(header, rows)
}

fn print_runs_as_table(runs: &[ExperimentRun<'_>]) -> Result<()> {
    let table = runs_to_table(runs);
    table.write_pretty(std::io::stdout())
}

fn print_runs_as_csv(runs: &[ExperimentRun<'_>]) -> Result<()> {
    let table = runs_to_table(runs);
    table.write_csv(std::io::stdout())
}

fn print_runs_as_json(runs: &[ExperimentRun<'_>]) -> Result<()> {
    let as_serializable: Vec<SerializableRun> = runs.iter().map(|r| r.into()).collect();
    print_serializable_as_json(&as_serializable)
}

fn print_runs_as_yaml(runs: &[ExperimentRun<'_>]) -> Result<()> {
    let as_serializable: Vec<SerializableRun> = runs.iter().map(|r| r.into()).collect();
    print_serializable_as_yaml(&as_serializable)
}
