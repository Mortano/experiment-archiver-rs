use std::{collections::HashSet, ops::Range, sync::Mutex};

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use itertools::Itertools;
use postgres::{Client, Config, NoTls, Row, Transaction};

use crate::{
    experiment::{ExperimentInstance, ExperimentVersion},
    run::ExperimentRun,
    variable::{DataType, Variable, VariableType, VariableValue},
};

use super::Database;

// Edge cases:
// 1) What if a variable with the same name now has different types or descriptions?
// 2) Could a variable that is an input variable in one experiment be an output variable in another? I think yes...
//    Which means that we can't use the name as a PKEY for the variable, but instead have to use an ID or something...
// 3) If we overwrite a version of an experiment (e.g. 'latest') with new variables, we must delete the old
//    mappings. Probably best achieved by first deleting the old version and then adding the version anew
// 4) Should variables have versioning as well?

const ENV_PSQL_USER: &str = "PSQL_USER";
const ENV_PSQL_PWD: &str = "PSQL_PWD";
const ENV_PSQL_HOST: &str = "PSQL_HOST";
const ENV_PSQL_PORT: &str = "PSQL_PORT";
const ENV_PSQL_DBNAME: &str = "PSQL_DBNAME";

/// Returns connection config for postgres DB, fetched from environment variables
pub(crate) fn get_postgres_config() -> Result<Config> {
    let mut config = Client::configure();
    config
        .host(
            std::env::var(ENV_PSQL_HOST)
                .context("Could not get host name for postgres connection")?
                .as_str(),
        )
        .port(
            std::env::var(ENV_PSQL_PORT)
                .context("Could not get port for postgres connection")?
                .as_str()
                .parse()
                .context(format!(
                    "Value of {ENV_PSQL_PORT} must be a valid port number"
                ))?,
        )
        .user(
            std::env::var(ENV_PSQL_USER)
                .context("Could not user host name for postgres connection")?
                .as_str(),
        )
        .password(
            std::env::var(ENV_PSQL_PWD)
                .context("Could not get password for postgres connection")?
                .as_str(),
        )
        .dbname(
            std::env::var(ENV_PSQL_DBNAME)
                .context("Could not get database name for postgres connection")?
                .as_str(),
        );
    Ok(config)
}

pub(crate) struct PostgresClient {
    client: Mutex<Client>,
}

impl PostgresClient {
    pub(crate) fn connect() -> Result<Self> {
        let config =
            get_postgres_config().context("Can't get connection configuration for postgres DB")?;
        let client = config.connect(NoTls).context(format!(
            "Could not connect to postgres DB with config {:?}",
            config
        ))?;
        Ok(Self {
            client: Mutex::new(client),
        })
    }

    fn parse_rows_to_runs<'a>(
        rows: Vec<Row>,
        experiment_instance: &'a ExperimentInstance<'a>,
    ) -> Result<Vec<ExperimentRun<'a>>> {
        let mut runs = vec![];
        for (run_id, measurements) in &rows.iter().group_by(|row| row.get::<_, &str>("id")) {
            let mut date: Option<DateTime<Utc>> = None;
            let variable_values = measurements.map(|row| -> Result<VariableValue<'a>> {
                // Hack: Get the date of the run while iterating over all measurement rows
                date = row.try_get("date").context("Missing entry 'date' in row")?;

                let name: &str = row.try_get("var_name").context("Missing entry 'var_name' in row")?;
                let value: &str = row.try_get("value").context("Missing entry 'value' in row")?;
                let matching_variable = experiment_instance.experiment_version().output_variables().iter().find(|out_var| out_var.name() == name).ok_or_else(|| anyhow!("Unexpected variable {name} in measurements of run {run_id}"))?;
                let parsed_value = matching_variable.data_type().parse_str_as_generic_value(value).with_context(|| format!("Value {value} does not match expected data type of variable {name}"))?;
                Ok(VariableValue::from_variable(matching_variable, parsed_value))
            }).collect::<Result<Vec<_>>>().with_context(|| format!("Failed to parse measurements for run {run_id} of experiment instance {}", experiment_instance.id()))?;

            runs.push(ExperimentRun::new(
                experiment_instance,
                date.expect("Missing date for run"),
                run_id.to_string(),
                variable_values,
            ))
        }

        Ok(runs)
    }

    // fn overwrite_experiment_version(
    //     experiment_version: &ExperimentVersion,
    //     transaction: &mut Transaction<'_>,
    // ) -> Result<()> {
    //     // Delete old experiment version with all linked data, then insert new version
    //     Self::delete_experiment_version(experiment_version.id(), transaction).with_context(
    //         || {
    //             format!(
    //                 "Failed to delete data for old experiment {} @ version {}",
    //                 experiment_version.experiment().name(),
    //                 experiment_version.version()
    //             )
    //         },
    //     )?;

    //     Self::add_new_experiment_version(experiment_version, transaction)
    // }

    fn add_new_experiment_version(
        experiment_version: &ExperimentVersion,
        transaction: &mut Transaction<'_>,
    ) -> Result<()> {
        let all_variables = experiment_version
            .input_variables()
            .iter()
            .chain(experiment_version.output_variables().iter())
            .collect::<Vec<_>>();

        // We won't overwrite existing variables (i.e. same name, different type or description) as this could
        // alter data for other experiments. But we have to disallow changing existing variables when creating a
        // new `ExperimentVersion`!
        for variable in &all_variables {
            let data_type = serde_json::to_string(&variable.data_type())?;
            transaction
                .execute(
                    "INSERT INTO variables (name, description, type) 
                            VALUES ($1, $2, $3) 
                            ON CONFLICT (name) DO NOTHING",
                    &[&variable.name(), &variable.description(), &data_type],
                )
                .with_context(|| format!("Failed to insert variable {variable} into database"))?;
        }

        // Safe to add this version because if it would have existed, `delete_experiment_version` would have been
        // called previously to cleanup the old version
        let researchers_joined = experiment_version.researchers().iter().join(";");
        transaction
            .execute(
                "INSERT INTO experiment_versions (id, name, version, date, description, researchers) VALUES ($1, $2, $3, $4, $5, $6)",
                &[
                    &experiment_version.id(),
                    &experiment_version.name(),
                    &experiment_version.version(),
                    &experiment_version.date(),
                    &experiment_version.description(),
                    &researchers_joined,
                ],
            )
            .with_context(|| {
                format!(
                    "Failed to insert experiment version {} into database",
                    experiment_version.version()
                )
            })?;

        // Link variables to experiment
        for (kind, variable) in experiment_version
            .input_variables()
            .iter()
            .map(|v| (VariableType::Input, v))
            .chain(
                experiment_version
                    .output_variables()
                    .iter()
                    .map(|v| (VariableType::Output, v)),
            )
        {
            let kind_json = serde_json::to_string(&kind)?;
            // If we already have a link between the experiment version and the variable, leave it
            transaction.execute("INSERT INTO experiment_variables (var_name, ex_name, ex_version_id, kind) VALUES ($1, $2, $3, $4) ON CONFLICT (var_name, ex_version_id) DO NOTHING", &[
                &variable.name(),
                &experiment_version.name(),
                &experiment_version.id(),
                &kind_json,
            ]).with_context(|| format!("Failed to link variable {} to experiment {} at version {}", variable.name(), experiment_version.name(), experiment_version.version()))?;
        }

        Ok(())
    }

    fn delete_experiment_version(
        experiment_version_id: &str,
        transaction: &mut Transaction<'_>,
    ) -> Result<()> {
        // Remove variable links
        transaction.execute("DELETE FROM experiment_variables WHERE ex_version_id = $1", &[&experiment_version_id]).with_context(|| format!("Failed to unlink variables from old experiment version {experiment_version_id}"))?;

        // Remove all existing ExperimentInstances for the old version, which means:
        // 1) Deleting all measurements for runs for instances that belong to this version
        transaction.execute("DELETE FROM measurements M 
        USING runs R, experiment_instances I 
        WHERE M.run_id = R.id AND R.ex_instance_id = I.id AND I.version_id = $1", &[&experiment_version_id]).with_context(|| format!("Failed to delete measurements for old experiment version {experiment_version_id}"))?;
        // 2) Deleting all runs for instances that belong to this version
        transaction.execute("DELETE FROM runs R 
        USING experiment_instances I 
        WHERE R.ex_instance_id = I.id AND I.version_id = $1", &[&experiment_version_id]).with_context(|| format!("Failed to delete runs for instances of old experiment version {experiment_version_id}"))?;
        // 3) Deleting all in_values for instances that belong to this version
        transaction.execute("DELETE FROM in_values V
        USING experiment_instances I 
        WHERE V.ex_instance_id = I.id AND I.version_id = $1", &[&experiment_version_id]).with_context(|| format!("Failed to delete in_values for instances of old experiment version {experiment_version_id}"))?;
        // 4) Deleting all instances that belong to this version
        transaction
            .execute(
                "DELETE FROM experiment_instances WHERE version_id = $1",
                &[&experiment_version_id],
            )
            .with_context(|| {
                format!(
                    "Failed to delete instances of old experiment version {experiment_version_id}"
                )
            })?;

        // Delete the actual ExperimentVersion
        transaction
            .execute(
                "DELETE FROM experiment_versions WHERE id = $1",
                &[&experiment_version_id],
            )
            .with_context(|| {
                format!("Failed to delete old experiment version {experiment_version_id}")
            })?;

        Ok(())
    }

    fn delete_experiment_instance(
        instance_id: &str,
        transaction: &mut Transaction<'_>,
    ) -> Result<()> {
        // 1) Deleting all measurements for runs that belong to this instance
        transaction
            .execute(
                "DELETE FROM measurements M 
        USING runs R
        WHERE M.run_id = R.id AND R.ex_instance_id = $1",
                &[&instance_id],
            )
            .with_context(|| format!("Failed to delete measurements for instance {instance_id}"))?;
        // 2) Deleting all runs that belong to this instance
        transaction
            .execute(
                "DELETE FROM runs R 
        WHERE R.ex_instance_id = $1",
                &[&instance_id],
            )
            .with_context(|| format!("Failed to delete runs for instance {instance_id}"))?;
        // 3) Deleting all in_values for this instance
        transaction
            .execute(
                "DELETE FROM in_values V
        WHERE V.ex_instance_id = $1",
                &[&instance_id],
            )
            .with_context(|| format!("Failed to delete in_values for instance {instance_id}"))?;
        // 4) Delete the actual instance
        transaction
            .execute(
                "DELETE FROM experiment_instances WHERE id = $1",
                &[&instance_id],
            )
            .with_context(|| format!("Failed to delete experiment instance {instance_id}"))?;
        Ok(())
    }
}

impl Database for PostgresClient {
    fn fetch_experiments(&self) -> Result<Vec<String>> {
        let mut client = self.client.lock().expect("Lock was poisoned");
        let rows = client
            .query("SELECT name FROM experiments", &[])
            .context("Failed to execute SQL query")?;

        Ok(rows
            .iter()
            .map(|row| -> String { row.get("name") })
            .collect_vec())
    }

    fn fetch_latest_experiment_version_by_name(
        &self,
        name: &str,
    ) -> Result<Option<ExperimentVersion>> {
        let mut all_versions = self.fetch_all_experiment_versions_by_name(name)?;
        all_versions.sort_by(|a, b| b.date().cmp(a.date()));
        if all_versions.is_empty() {
            Ok(None)
        } else {
            Ok(Some(all_versions.remove(0)))
        }
    }

    fn fetch_all_experiment_versions_by_name(&self, name: &str) -> Result<Vec<ExperimentVersion>> {
        let mut client = self.client.lock().expect("Lock was poisoned");
        let rows = client
            .query(
                "SELECT experiments.name, description, researchers, version, date, id
                FROM experiment_versions 
                INNER JOIN experiments 
                ON experiment_versions.name = experiments.name 
                WHERE experiments.name = $1",
                &[&name],
            )
            .context("Failed to execute SQL query")?;

        rows.iter().map(|row| -> Result<ExperimentVersion> {
            let experiment_name: String = row.get("name");
                let experiment_version: String = row.get("version");
                let experiment_date = row.get("date");
                let experiment_version_id = row.get("id");
                let experiment_description = row.get("description");
                let experiment_researchers_str: &str = row.get("researchers");
                let experiment_researchers = experiment_researchers_str.split(";").map(|s| s.to_string()).collect();

                // Fetch all variables that match the experiment
                let variable_rows = client
                    .query(
                        "SELECT variables.name, variables.description, experiment_variables.kind, variables.type
                                FROM variables 
                                INNER JOIN experiment_variables 
                                ON experiment_variables.var_name = variables.name 
                                WHERE experiment_variables.ex_name = $1 AND experiment_variables.ex_version_id = $2",
                        &[&experiment_name, &experiment_version_id],
                    )
                    .context("Failed to fetch variables")?;

                let mut input_variables = HashSet::default();
                let mut output_variables = HashSet::default();

                for row in variable_rows {
                    let var: Variable = (&row).try_into().with_context(|| {
                        format!("Failed to parse Variable from SQL row {row:?}")
                    })?;
                    let variable_type_str: &str = row.get("kind");
                    let variable_type: VariableType = serde_json::from_str(variable_type_str).with_context(|| format!("Failed to parse VariableType from SQL value {variable_type_str}"))?;
                    if variable_type == VariableType::Input {
                        input_variables.insert(var);
                    } else {
                        output_variables.insert(var);
                    }
                }

                let experiment_version = ExperimentVersion::from_experiment(
                    experiment_version_id,
                    experiment_name,
                    experiment_version,
                    experiment_date,
                    experiment_description,
                    experiment_researchers,
                    input_variables,
                    output_variables,
                );
                Ok(experiment_version)
        }).collect()
    }

    fn fetch_specific_experiment_version(
        &self,
        name: &str,
        version: &str,
    ) -> Result<Option<ExperimentVersion>> {
        let all_versions = self.fetch_all_experiment_versions_by_name(name)?;
        let specific_version = all_versions.into_iter().find(|ex| ex.version() == version);
        Ok(specific_version)
    }

    fn fetch_all_instances_of_experiment_version<'a>(
        &self,
        experiment_version: &'a ExperimentVersion,
    ) -> Result<Vec<ExperimentInstance<'a>>> {
        let mut client = self.client.lock().expect("Lock was poisoned");

        let rows = client
            .query(
                "SELECT id, var_name, value 
                        FROM in_values 
                        INNER JOIN experiment_instances 
                        ON in_values.ex_instance_id = experiment_instances.id 
                        WHERE experiment_instances.name = $1 AND experiment_instances.version_id = $2",
                &[
                    &experiment_version.name(),
                    &experiment_version.id(),
                ],
            )
            .context("Failed to execute SQL query")?;

        drop(client);

        let mut instances = vec![];
        for (instance_id, group) in &rows.iter().group_by(|row| row.get::<_, &str>("id")) {
            let variable_values = group.map(|row| -> Result<VariableValue<'a>> {
                let name: &str = row.try_get("var_name").context("Missing entry 'var_name' in row")?;
                let value: &str = row.try_get("value").context("Missing entry 'value' in row")?;
                let matching_variable = experiment_version.input_variables().iter().find(|in_var| in_var.name() == name).ok_or_else(|| anyhow!("Unexpected variable {name} in input values of experiment instance {instance_id}"))?;
                let parsed_value = matching_variable.data_type().parse_str_as_generic_value(value).with_context(|| format!("Value {value} does not match expected data type of variable {name}"))?;
                Ok(VariableValue::from_variable(matching_variable, parsed_value))
            }).collect::<Result<Vec<_>>>().with_context(|| format!("Failed to parse input variables for experiment instance with ID {instance_id}"))?;

            instances.push(ExperimentInstance::new(
                experiment_version,
                variable_values,
                instance_id.to_string(),
            ));
        }

        Ok(instances)
    }

    fn fetch_specific_instance<'a>(
        &self,
        experiment_version: &'a ExperimentVersion,
        input_variable_values: &[VariableValue<'a>],
    ) -> Result<Option<ExperimentInstance<'a>>> {
        // Just fetch all instances and look for a matching one instead of doing SQL gymnastics
        let all_instances = self
            .fetch_all_instances_of_experiment_version(experiment_version)
            .context("Failed to fetch instances from database")?;

        let matching_instance = all_instances.into_iter().find(|instance| {
            let actual_variable_values = instance.input_variable_values();
            if actual_variable_values.len() != input_variable_values.len() {
                return false;
            }
            actual_variable_values.iter().all(|actual_value| {
                input_variable_values
                    .iter()
                    .find(|expected_value| *expected_value == actual_value)
                    .is_some()
            })
        });

        Ok(matching_instance)
    }

    fn fetch_all_runs_of_instance<'a>(
        &self,
        experiment_instance: &'a ExperimentInstance<'a>,
    ) -> Result<Vec<ExperimentRun<'a>>> {
        let mut client = self.client.lock().expect("Lock was poisoned");

        let rows = client
            .query(
                "SELECT id, date, var_name, value 
                        FROM runs 
                        INNER JOIN measurements 
                        ON measurements.run_id = runs.id
                        WHERE runs.ex_instance_id = $1",
                &[&experiment_instance.id()],
            )
            .context("Failed to execute SQL query")?;

        Self::parse_rows_to_runs(rows, experiment_instance)
    }

    fn fetch_runs_in_date_range<'a>(
        &self,
        experiment_instance: &'a ExperimentInstance<'a>,
        date_range: Range<DateTime<Utc>>,
    ) -> Result<Vec<ExperimentRun<'a>>> {
        let mut client = self.client.lock().expect("Lock was poisoned");

        let rows = client
            .query(
                "SELECT id, date, var_name, value 
                        FROM runs 
                        INNER JOIN measurements 
                        ON measurements.run_id = runs.id
                        WHERE runs.ex_instance_id = $1 AND runs.date >= $2 AND runs.date < $3",
                &[
                    &experiment_instance.id(),
                    &date_range.start,
                    &date_range.end,
                ],
            )
            .context("Failed to execute SQL query")?;

        Self::parse_rows_to_runs(rows, experiment_instance)
    }

    fn insert_new_experiment(&self, experiment: &ExperimentVersion) -> Result<()> {
        // Experiment is new and ExperimentVersion is new as well. Might mean that variables are also new
        let mut client = self.client.lock().expect("Lock was poisoned");
        let mut transaction = client
            .transaction()
            .context("Failed to begin transaction")?;

        transaction
            .execute(
                "INSERT INTO experiments (name) VALUES ($1)",
                &[&experiment.name()],
            )
            .with_context(|| {
                format!(
                    "Failed to insert experiment {} into database",
                    experiment.name(),
                )
            })?;

        Self::add_new_experiment_version(experiment, &mut transaction)?;

        transaction
            .commit()
            .context("Failed to execute transaction")?;

        Ok(())
    }

    fn insert_new_experiment_version(&self, experiment_version: &ExperimentVersion) -> Result<()> {
        // This assumes that `experiment_version` is actually a new version, so it performs no checks whether
        // this exact version already exists in the DB (the check should be done by the caller, e.g. in `Experiment::sync_with_db`)

        let mut client = self.client.lock().expect("Lock was poisoned");
        let mut transaction = client
            .transaction()
            .context("Failed to begin transaction")?;
        Self::add_new_experiment_version(experiment_version, &mut transaction)?;

        transaction
            .commit()
            .context("Failed to execute transaction")?;

        Ok(())
    }

    fn insert_new_experiment_instance(
        &self,
        experiment_instance: &ExperimentInstance<'_>,
    ) -> Result<()> {
        let mut client = self.client.lock().expect("Lock was poisoned");
        let mut transaction = client
            .transaction()
            .context("Failed to begin transaction")?;

        // Insert instance and new in_values
        transaction
            .execute(
                "INSERT INTO experiment_instances (id, name, version_id) VALUES ($1, $2, $3)",
                &[
                    &experiment_instance.id(),
                    &experiment_instance.experiment_version().name(),
                    &experiment_instance.experiment_version().id(),
                ],
            )
            .with_context(|| {
                format!(
                    "Failed to insert experiment instance {} for experiment {} at version {}",
                    experiment_instance.id(),
                    experiment_instance.experiment_version().name(),
                    experiment_instance.experiment_version().version()
                )
            })?;

        for in_value in experiment_instance.input_variable_values() {
            transaction
                .execute(
                    "INSERT INTO in_values (ex_instance_id, var_name, value) VALUES ($1, $2, $3)",
                    &[
                        &experiment_instance.id(),
                        &in_value.variable().name(),
                        &in_value.value().to_string(),
                    ],
                )
                .with_context(|| {
                    format!(
                        "Failed to insert fixed input value {:?} for experiment instance {}",
                        in_value,
                        experiment_instance.id()
                    )
                })?;
        }

        transaction
            .commit()
            .context("Failed to commit transaction")?;

        Ok(())
    }

    fn insert_new_run(&self, run: &ExperimentRun<'_>) -> Result<()> {
        let mut client = self.client.lock().expect("Lock was poisoned");
        let mut transaction = client
            .transaction()
            .context("Failed to begin transaction")?;

        // Insert run, then insert measurements and link to run
        transaction
            .execute(
                "INSERT INTO runs (ex_instance_id, id, date) VALUES ($1, $2, $3)",
                &[&run.experiment_instance().id(), &run.id(), &run.date()],
            )
            .with_context(|| format!("Failed to insert run {} into database", run.id()))?;

        for measurement in run.measurements() {
            transaction
                .execute(
                    "INSERT INTO measurements (run_id, var_name, value) VALUES ($1, $2, $3)",
                    &[
                        &run.id(),
                        &measurement.variable().name(),
                        &measurement.value().to_string(),
                    ],
                )
                .with_context(|| {
                    format!(
                        "Failed to insert measurement for variable {} of run {} into database",
                        measurement.variable().name(),
                        run.id()
                    )
                })?;
        }

        transaction
            .commit()
            .context("Failed to execute transaction")?;
        Ok(())
    }

    fn delete_experiment(&self, name: &str) -> Result<()> {
        let versions_of_experiment = self
            .fetch_all_experiment_versions_by_name(name)
            .with_context(|| format!("Failed to fetch all versions of experiment {name}"))?;
        if versions_of_experiment.is_empty() {
            return Ok(());
        }

        let mut client = self.client.lock().expect("Lock was poisoned");
        let mut transaction = client
            .transaction()
            .context("Failed to begin transaction")?;

        for version in versions_of_experiment {
            Self::delete_experiment_version(version.id(), &mut transaction).with_context(|| {
                format!(
                    "Failed to delete version {} of experiment {name}",
                    version.id()
                )
            })?;
        }

        transaction
            .execute("DELETE FROM experiments WHERE name = $1", &[&name])
            .with_context(|| format!("Failed to delete experiment {name}"))?;
        transaction
            .commit()
            .context("Failed to commit transaction")?;

        Ok(())
    }

    fn delete_experiment_version(&self, experiment_version: &ExperimentVersion) -> Result<()> {
        let mut client = self.client.lock().expect("Lock was poisoned");
        let mut transaction = client
            .transaction()
            .context("Failed to begin transaction")?;

        Self::delete_experiment_version(experiment_version.id(), &mut transaction)?;

        transaction
            .commit()
            .context("Failed to execute transaction")?;
        Ok(())
    }

    fn delete_experiment_instance(
        &self,
        experiment_instance: &ExperimentInstance<'_>,
    ) -> Result<()> {
        let mut client = self.client.lock().expect("Lock was poisoned");
        let mut transaction = client
            .transaction()
            .context("Failed to begin transaction")?;

        Self::delete_experiment_instance(experiment_instance.id(), &mut transaction)?;

        transaction
            .commit()
            .context("Failed to execute transaction")?;
        Ok(())
    }
}

impl TryFrom<&'_ Row> for Variable {
    type Error = anyhow::Error;

    fn try_from(row: &'_ Row) -> std::result::Result<Self, Self::Error> {
        let name = row.try_get("name")?;
        let description = row.try_get("description")?;
        let kind_str = row.try_get::<_, &str>("kind")?;
        let data_type_str = row.try_get::<_, &str>("type")?;

        let data_type: DataType = serde_json::from_str(data_type_str).with_context(|| {
            format!("Could not deserialize DataType from JSON string {kind_str}")
        })?;

        Ok(Self::new(name, description, data_type))
    }
}
