use crate::{
    generic_table::GenericTable,
    util::{print_serializable_as_json, print_serializable_as_yaml, SerializableVariableValue},
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

pub fn list_instances(version_id: &str, output_format: OutputFormat) -> Result<()> {
    let db = db_connection();
    let version = db
        .fetch_experiment_version_by_id(version_id)
        .with_context(|| format!("Failed to fetch experiment version {version_id}"))?
        .ok_or_else(|| anyhow!("No experiment version with ID {version_id} found"))?;
    let instances = db
        .fetch_all_instances_of_experiment_version(&version)
        .with_context(|| format!("Failed to fetch instances of experiment version {version_id}"))?;

    if instances.is_empty() {
        bail!(
            "No instances found for experiment {} @ version {}",
            version.name(),
            version.version()
        );
    }

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

fn instances_to_generic_table(instances: &[ExperimentInstance<'_>]) -> GenericTable {
    let variables = instances[0].experiment_version().input_variables();
    let mut sorted_variables = variables.iter().collect::<Vec<_>>();
    sorted_variables.sort_by(|a, b| a.name().cmp(b.name()));

    let header = ["Name", "Version"]
        .into_iter()
        .chain(sorted_variables.iter().map(|v| v.name()))
        .map(|s| s.to_string())
        .collect_vec();
    let rows = instances
        .iter()
        .map(|instance| -> Vec<String> {
            [
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
