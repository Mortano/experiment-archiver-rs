use std::{collections::HashSet, fmt::Display, io::Read};

use chrono::{DateTime, Utc};
use itertools::Itertools;

use anyhow::{anyhow, Context, Result};

use crate::{database::db_connection, run::RunContext, variable::GenericValue};

use super::{
    run::ExperimentRun,
    use_database,
    util::gen_unique_id,
    variable::{Variable, VariableValue},
    version::current_version,
};

#[derive(Debug, PartialEq, Clone)]
pub struct ExperimentVersion {
    id: String,
    name: String,
    version: String,
    date: DateTime<Utc>,
    description: String,
    researchers: HashSet<String>,
    input_variables: HashSet<Variable>,
    output_variables: HashSet<Variable>,
}

impl ExperimentVersion {
    /// Creates a new `ExperimentVersion` from the given parameters. The `name` field uniquely identifies an experiment
    /// in the database, all other fields are versioned based on the name (if the database connection is active, see
    /// [`use_database`] for more information)
    pub fn get_current_version(
        name: String,
        description: String,
        researchers: HashSet<String>,
        input_variables: HashSet<Variable>,
        output_variables: HashSet<Variable>,
    ) -> Result<ExperimentVersion> {
        if use_database() {
            Self::sync_with_db(
                name,
                description,
                researchers,
                input_variables,
                output_variables,
            )
            .with_context(|| "Failed to synchronize new experiment with database")
        } else {
            let version = current_version();
            let date = Utc::now();
            Ok(Self::from_experiment(
                gen_unique_id(),
                name,
                version,
                date,
                description,
                researchers,
                input_variables,
                output_variables,
            ))
        }
    }

    #[cfg(feature = "yaml")]
    pub fn from_yaml<R: Read>(reader: R) -> Result<ExperimentVersion> {
        use crate::util::yaml::YamlExt;

        let yaml_value: serde_yaml::Value = serde_yaml::from_reader(reader)?;
        let name = yaml_value.field_as_str("name")?;
        let description = yaml_value.field_as_str("description")?;
        let researchers = yaml_value.field_as_vec_str("researchers")?;
        let input_variables = yaml_value
            .field_as_sequence("input_variables")?
            .iter()
            .map(|v| Variable::from_yaml(v))
            .collect::<Result<HashSet<_>>>()
            .with_context(|| "Failed to parse input_variables field")?;
        let output_variables = yaml_value
            .field_as_sequence("output_variables")?
            .iter()
            .map(|v| Variable::from_yaml(v))
            .collect::<Result<HashSet<_>>>()
            .with_context(|| "Failed to parse output_variables field")?;

        Self::get_current_version(
            name.to_string(),
            description.to_string(),
            researchers.into_iter().map(|str| str.to_string()).collect(),
            input_variables,
            output_variables,
        )
    }

    /// Synchronize the experiment called `name` with the database. If this is a known experiment, this fetches the correct IDs
    /// from the database and potentially marks this experiment as a new version if something has changed (description,
    /// name of researchers, variables, build version or git commit hash (if the `version-from-git` feature is enabled)). If
    /// the experiment is not known, it is added to the database together with all its variables
    fn sync_with_db(
        name: String,
        description: String,
        researchers: HashSet<String>,
        input_variables: HashSet<Variable>,
        output_variables: HashSet<Variable>,
    ) -> Result<Self> {
        let db_connection = db_connection();
        let latest_version = db_connection
            .fetch_latest_experiment_version_by_name(&name)
            .context("Failed to fetch latest version of experiment from database")?;
        if let Some(latest_version) = latest_version {
            // Check if there is a difference between the current version and the latest version in the DB!
            let current_version = current_version();
            if current_version != latest_version.version()
                || *latest_version.description() != description
                || *latest_version.researchers() != researchers
                || *latest_version.input_variables() != input_variables
                || *latest_version.output_variables() != output_variables
            {
                let date = Utc::now();
                let new_experiment_version = ExperimentVersion::from_experiment(
                    gen_unique_id(),
                    name,
                    current_version,
                    date,
                    description,
                    researchers,
                    input_variables,
                    output_variables,
                );
                db_connection
                    .insert_new_experiment_version(&new_experiment_version)
                    .context("Failed to insert new version of experiment into database")?;
                Ok(new_experiment_version)
            } else {
                Ok(latest_version)
            }
        } else {
            // Insert as new experiment version!
            let version = current_version();
            let date = Utc::now();
            let experiment_version = ExperimentVersion::from_experiment(
                gen_unique_id(),
                name,
                version,
                date,
                description,
                researchers,
                input_variables,
                output_variables,
            );

            db_connection
                .insert_new_experiment(&experiment_version)
                .context("Failed to insert new experiment into database")?;

            Ok(experiment_version)
        }
    }

    pub(crate) fn from_experiment(
        id: String,
        name: String,
        version: String,
        date: DateTime<Utc>,
        description: String,
        researchers: HashSet<String>,
        input_variables: HashSet<Variable>,
        output_variables: HashSet<Variable>,
    ) -> Self {
        Self {
            id,
            name,
            date,
            version,
            description,
            researchers,
            input_variables,
            output_variables,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn date(&self) -> &DateTime<Utc> {
        &self.date
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn researchers(&self) -> &HashSet<String> {
        &self.researchers
    }

    pub fn input_variables(&self) -> &HashSet<Variable> {
        &self.input_variables
    }

    pub fn input_variable_by_name(&self, name: &str) -> Option<&Variable> {
        self.input_variables.iter().find(|v| v.name() == name)
    }

    pub fn output_variables(&self) -> &HashSet<Variable> {
        &self.output_variables
    }

    pub fn output_variable_by_name(&self, name: &str) -> Option<&Variable> {
        self.output_variables.iter().find(|v| v.name() == name)
    }

    /// Create an `ExperimentInstance` by fixing the input variables of this `ExperimentVersion` to the given values. An
    /// `ExperimentInstance` is required to perform actual measurements (by calling [`ExperimentInstance::run`])
    pub fn make_instance<'a, 'b, I: IntoIterator<Item = (&'b str, GenericValue)>>(
        &'a self,
        input_variable_values: I,
    ) -> Result<ExperimentInstance<'a>> {
        let as_variable_values = input_variable_values
            .into_iter()
            .map(|(name, value)| -> Result<VariableValue<'a>> {
                self.input_variable_by_name(name)
                    .ok_or_else(|| anyhow!("No variable with name {name} found"))
                    .map(|var| VariableValue::from_variable(var, value))
            })
            .collect::<Result<Vec<_>>>()?;
        ExperimentInstance::from_experiment_version(self, as_variable_values)
    }
}

impl Display for ExperimentVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Name:                  {}", self.name)?;
        writeln!(f, "Description:           {}", self.description)?;
        writeln!(f, "ID:                    {}", self.id)?;
        writeln!(f, "Version:               {}", self.version)?;
        writeln!(f, "Date:                  {}", self.date)?;
        writeln!(
            f,
            "Researchers:           {}",
            self.researchers.iter().join(";")
        )?;

        let in_variables_str = self.input_variables.iter().map(|v| v.to_string()).join(";");
        let out_variables_str = self
            .output_variables
            .iter()
            .map(|v| v.to_string())
            .join(";");
        writeln!(f, "Variables (input):     {}", in_variables_str)?;
        writeln!(f, "Variables (output):    {}", out_variables_str)?;

        Ok(())
    }
}

#[derive(Debug)]
pub struct ExperimentInstance<'a> {
    experiment_version: &'a ExperimentVersion,
    input_variable_values: Vec<VariableValue<'a>>,
    id: String,
}

impl<'a> ExperimentInstance<'a> {
    pub(crate) fn from_experiment_version(
        experiment_version: &'a ExperimentVersion,
        input_variable_values: Vec<VariableValue<'a>>,
    ) -> Result<Self> {
        let expected_variable_names = experiment_version
            .input_variables()
            .iter()
            .map(|v| v.name())
            .collect::<HashSet<_>>();
        let actual_variable_names = input_variable_values
            .iter()
            .map(|v| v.variable().name())
            .collect::<HashSet<_>>();
        if expected_variable_names != actual_variable_names {
            let name_to_variable = |name: &&str| -> String {
                let matching_variable = experiment_version
                    .input_variables()
                    .iter()
                    .find(|v| v.name() == *name)
                    .unwrap();
                format!("{matching_variable:?}")
            };
            let missing_variables = expected_variable_names
                .difference(&actual_variable_names)
                .map(name_to_variable)
                .join(", ");
            let unexpected_variables = actual_variable_names
                .difference(&expected_variable_names)
                .map(name_to_variable)
                .join(", ");
            panic!("Input variable values do not match the set of input variables of the experiment. Missing the following variables:\n{missing_variables}\nFound the following unexpected variables:\n{unexpected_variables}");
        }

        if use_database() {
            // Sync with DB to check if there is an instance with exactly the same input variable values. If so,
            // use the instance from the DB, otherwise this is a new instance!
            let db_client = db_connection();
            let maybe_instance = db_client
                .fetch_specific_instance(experiment_version, &input_variable_values)
                .context("Failed to fetch specific experiment instance from database")?;

            if let Some(existing_instance) = maybe_instance {
                Ok(existing_instance)
            } else {
                let new_id = gen_unique_id();
                let new_instance = Self {
                    experiment_version,
                    input_variable_values,
                    id: new_id,
                };
                db_client
                    .insert_new_experiment_instance(&new_instance)
                    .context("Failed to insert new experiment instance into database")?;
                Ok(new_instance)
            }
        } else {
            // If DB is off, create generic unique ID and be done with it
            Ok(Self {
                experiment_version,
                input_variable_values,
                id: gen_unique_id(),
            })
        }
    }

    pub(crate) fn new(
        experiment_version: &'a ExperimentVersion,
        input_variable_values: Vec<VariableValue<'a>>,
        id: String,
    ) -> Self {
        Self {
            experiment_version,
            id,
            input_variable_values,
        }
    }

    pub fn run<F: FnOnce(&RunContext) -> Result<()>>(
        &'a self,
        run_fn: F,
    ) -> Result<ExperimentRun<'a>> {
        let context = RunContext::from_instance(self);
        run_fn(&context).context("Failed to execute experiment run function")?;

        let run = context.to_run().context("Failed to get data for run")?;
        run.log();

        if use_database() {
            run.sync_with_db()
                .context("Failed to write data for experiment run to database")?;
        }

        Ok(run)
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn experiment_version(&self) -> &ExperimentVersion {
        self.experiment_version
    }

    pub fn input_variable_values(&self) -> &[VariableValue<'a>] {
        &self.input_variable_values
    }
}
