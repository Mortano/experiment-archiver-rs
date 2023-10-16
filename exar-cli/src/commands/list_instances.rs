use crate::{
    generic_table::GenericTable,
    statistics::{aggregate_runs, RunStatistics},
    util::{
        print_serializable_as_json, print_serializable_as_yaml, variable_to_table_display,
        SerializableVariableValue,
    },
    OutputFormat,
};
use anyhow::{anyhow, bail, Context, Result};
use exar::{database::db_connection, experiment::ExperimentInstance};
use itertools::Itertools;
use serde::Serialize;

#[derive(Debug, Serialize)]
struct SerializableInstance {
    experiment_name: String,
    experiment_version: String,
    instance_id: String,
    variable_values: Vec<SerializableVariableValue>,
}

impl From<&ExperimentInstance<'_>> for SerializableInstance {
    fn from(value: &ExperimentInstance<'_>) -> Self {
        let mut variable_values: Vec<SerializableVariableValue> = value
            .input_variable_values()
            .iter()
            .map(|v| v.into())
            .collect_vec();
        variable_values.sort_by(|a, b| a.name.cmp(&b.name));
        Self {
            experiment_name: value.experiment_version().name().to_string(),
            experiment_version: value.experiment_version().version().to_string(),
            instance_id: value.id().to_string(),
            variable_values,
        }
    }
}

pub fn list_instances(
    version_id_or_name: &str,
    output_format: OutputFormat,
    statistics: bool,
    latest: bool,
) -> Result<()> {
    let db = db_connection();
    let version = if latest {
        let all_versions = db
            .fetch_all_experiment_versions_by_name(version_id_or_name)
            .with_context(|| {
                format!("Failed to fetch versions for experiment {version_id_or_name}")
            })?;
        all_versions
            .into_iter()
            .max_by(|a, b| a.date().cmp(b.date()))
            .ok_or_else(|| anyhow!("No versions found for experiment {version_id_or_name}"))?
    } else {
        db.fetch_experiment_version_by_id(version_id_or_name)
            .with_context(|| format!("Failed to fetch experiment version {version_id_or_name}"))?
            .ok_or_else(|| anyhow!("No experiment version with ID {version_id_or_name} found"))?
    };
    let instances = db
        .fetch_all_instances_of_experiment_version(&version)
        .with_context(|| {
            format!(
                "Failed to fetch instances of experiment version {}",
                version.id()
            )
        })?;

    if instances.is_empty() {
        bail!(
            "No instances found for experiment {} @ version {}",
            version.name(),
            version.version()
        );
    }

    if statistics {
        // Get all runs for each instance
        let run_stats_per_instance = instances
            .iter()
            .map(|instance| -> Result<Option<_>> {
                let runs = db.fetch_all_runs_of_instance(instance).with_context(|| {
                    format!("Failed to fetch runs for instance {}", instance.id())
                })?;
                if runs.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(aggregate_runs(&runs)))
                }
            })
            .collect::<Result<Vec<_>>>()?;
        // There might be instances that don't have runs. We filter these and only print stuff that has runs
        let instances_that_have_runs = run_stats_per_instance
            .iter()
            .enumerate()
            .filter_map(|(idx, maybe_runs)| {
                if maybe_runs.is_none() {
                    None
                } else {
                    Some(instances[idx].clone())
                }
            })
            .collect_vec();
        let existing_runs = run_stats_per_instance
            .into_iter()
            .filter_map(|maybe_run| maybe_run)
            .collect_vec();

        match output_format {
            OutputFormat::Table => {
                print_instances_and_runs_as_table(&instances_that_have_runs, &existing_runs)
            }
            OutputFormat::CSV => {
                print_instances_and_runs_as_csv(&instances_that_have_runs, &existing_runs)
            }
            OutputFormat::JSON => {
                print_instances_and_runs_as_json(&instances_that_have_runs, &existing_runs)
            }
            OutputFormat::YAML => {
                print_instances_and_runs_as_yaml(&instances_that_have_runs, &existing_runs)
            }
        }
    } else {
        match output_format {
            OutputFormat::Table => print_instances_as_table(&instances),
            OutputFormat::CSV => print_instances_as_csv(&instances),
            OutputFormat::JSON => {
                let serializable: Vec<SerializableInstance> =
                    instances.iter().map(|i| i.into()).collect_vec();
                print_serializable_as_json(&serializable)
            }
            OutputFormat::YAML => {
                let serializable: Vec<SerializableInstance> =
                    instances.iter().map(|i| i.into()).collect_vec();
                print_serializable_as_yaml(&serializable)
            }
        }
    }
}

fn instances_to_generic_table(instances: &[ExperimentInstance<'_>]) -> GenericTable {
    let variables = instances[0].experiment_version().input_variables();
    let mut sorted_variables = variables.iter().collect::<Vec<_>>();
    sorted_variables.sort_by(|a, b| a.name().cmp(b.name()));

    let header = ["ID".to_string(), "Name".to_string(), "Version".to_string()]
        .into_iter()
        .chain(
            sorted_variables
                .iter()
                .map(|v| variable_to_table_display(*v)),
        )
        .collect_vec();
    let rows = instances
        .iter()
        .map(|instance| -> Vec<String> {
            [
                instance.id().to_string(),
                instance.experiment_version().name().to_string(),
                instance.experiment_version().version().to_string(),
            ]
            .into_iter()
            .chain(sorted_variables.iter().map(|v| {
                let matching_var_value = instance
                    .input_variable_values()
                    .iter()
                    .find(|val| val.variable() == *v)
                    .unwrap();
                matching_var_value.value().to_string()
            }))
            .collect()
        })
        .collect_vec();

    GenericTable::new(header, rows)
}

fn print_instances_as_table(instances: &[ExperimentInstance<'_>]) -> Result<()> {
    let table = instances_to_generic_table(instances);
    table.write_pretty(std::io::stdout())
}

fn print_instances_as_csv(instances: &[ExperimentInstance<'_>]) -> Result<()> {
    let table = instances_to_generic_table(instances);
    table.write_csv(std::io::stdout())
}

fn instances_and_runs_to_table(
    instances: &[ExperimentInstance<'_>],
    run_stats: &[RunStatistics],
) -> GenericTable {
    let mut instance_table = instances_to_generic_table(instances);
    let runs_table = GenericTable::new(
        run_stats[0].table_header(),
        run_stats.iter().map(|stats| stats.table_row()).collect(),
    );
    instance_table.append(runs_table);
    instance_table
}

fn print_instances_and_runs_as_table(
    instances: &[ExperimentInstance<'_>],
    run_stats: &[RunStatistics],
) -> Result<()> {
    let table = instances_and_runs_to_table(instances, run_stats);
    table.write_pretty(std::io::stdout())
}

fn print_instances_and_runs_as_csv(
    instances: &[ExperimentInstance<'_>],
    run_stats: &[RunStatistics],
) -> Result<()> {
    let table = instances_and_runs_to_table(instances, run_stats);
    table.write_csv(std::io::stdout())
}

#[derive(Debug, Serialize)]
struct SerializableInstanceAndRun {
    instance_id: String,
    input_values: Vec<SerializableVariableValue>,
    statistics: RunStatistics,
}

fn instances_and_runs_as_serializables(
    instances: &[ExperimentInstance<'_>],
    run_stats: &[RunStatistics],
) -> Vec<SerializableInstanceAndRun> {
    instances
        .iter()
        .zip(run_stats.iter())
        .map(|(instance, stats)| SerializableInstanceAndRun {
            input_values: instance
                .input_variable_values()
                .iter()
                .map(|v| v.into())
                .collect(),
            instance_id: instance.id().to_string(),
            statistics: stats.clone(),
        })
        .collect_vec()
}

fn print_instances_and_runs_as_json(
    instances: &[ExperimentInstance<'_>],
    run_stats: &[RunStatistics],
) -> Result<()> {
    let serializables = instances_and_runs_as_serializables(instances, run_stats);
    print_serializable_as_json(&serializables)
}

fn print_instances_and_runs_as_yaml(
    instances: &[ExperimentInstance<'_>],
    run_stats: &[RunStatistics],
) -> Result<()> {
    let serializables = instances_and_runs_as_serializables(instances, run_stats);
    print_serializable_as_yaml(&serializables)
}
