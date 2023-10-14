use std::{collections::HashSet, fmt::Display, sync::Mutex};

use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use itertools::Itertools;
use log::{info, LevelFilter};
use serde_json::json;

use crate::{database::db_connection, util::gen_unique_id};

use super::{
    experiment::ExperimentInstance,
    variable::{GenericValue, VariableValue},
};

/// A specific run of an `ExperimentInstance`. This contains all measured variables, the `DateTime` of the
/// execution of this run, as well as a reference to the `ExperimentInstance` that the run belongs to
#[derive(Debug)]
pub struct ExperimentRun<'a> {
    experiment_instance: &'a ExperimentInstance<'a>,
    date: DateTime<Utc>,
    id: String,
    measurements: Vec<VariableValue<'a>>,
}

impl<'a> ExperimentRun<'a> {
    pub(crate) fn new(
        experiment_instance: &'a ExperimentInstance<'a>,
        date: DateTime<Utc>,
        id: String,
        measurements: Vec<VariableValue<'a>>,
    ) -> Self {
        Self {
            experiment_instance,
            date,
            id,
            measurements,
        }
    }

    pub fn experiment_instance(&self) -> &ExperimentInstance<'a> {
        self.experiment_instance
    }

    pub fn date(&self) -> &DateTime<Utc> {
        &self.date
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn measurements(&self) -> &[VariableValue<'a>] {
        &self.measurements
    }

    pub(crate) fn sync_with_db(&self) -> Result<()> {
        let db = db_connection();
        db.insert_new_run(self)
    }

    pub(crate) fn log(&self) {
        if log::max_level() < LevelFilter::Info {
            return;
        }

        let as_json = self.to_json();
        let as_pretty_json = serde_json::to_string_pretty(&as_json)
            .expect("Failed to pretty-print ExperimentRun as JSON");
        info!("{as_pretty_json}");
    }

    fn to_json(&self) -> serde_json::Value {
        let input_values = self
            .experiment_instance
            .input_variable_values()
            .iter()
            .map(|val| [val.variable().name().to_string(), val.value().to_string()])
            .collect::<Vec<_>>();
        let measurements = self
            .measurements
            .iter()
            .map(|val| [val.variable().name().to_string(), val.value().to_string()])
            .collect::<Vec<_>>();
        json!({
            "name": self.experiment_instance.experiment_version().name(),
            "version": self.experiment_instance.experiment_version().version(),
            "date": self.date.to_string(),
            "input_variables": input_values,
            "measurements": measurements,
        })
    }
}

impl Display for ExperimentRun<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let in_out_variable_values = self
            .experiment_instance()
            .input_variable_values()
            .iter()
            .chain(self.measurements().iter())
            .map(|v| v.to_string())
            .join(" ");
        write!(f, "{} - {}", self.id, in_out_variable_values)
    }
}

/// Context object that is passed to `ExperimentInstance::run` through which measurements for the run
/// can be recorded
pub struct RunContext<'a> {
    experiment_instance: &'a ExperimentInstance<'a>,
    measurements: Mutex<Vec<VariableValue<'a>>>,
}

impl<'a> RunContext<'a> {
    pub(crate) fn from_instance(experiment_instance: &'a ExperimentInstance<'a>) -> Self {
        Self {
            experiment_instance,
            measurements: Default::default(),
        }
    }

    pub(crate) fn to_run(self) -> Result<ExperimentRun<'a>> {
        // Check that we have exactly one measurement for each output variable
        let measurements = self.measurements.into_inner().expect("Lock was poisoned");
        let measured_variables = measurements
            .iter()
            .map(|m| m.variable())
            .collect::<HashSet<_>>();
        let expected_variables = self
            .experiment_instance
            .experiment_version()
            .output_variables()
            .iter()
            .collect::<HashSet<_>>();
        if measured_variables != expected_variables {
            let missing_variables = expected_variables
                .difference(&measured_variables)
                .map(|v| v.name())
                .join(",");
            bail!("Missing measurements for output variables: {missing_variables}");
        }

        let date = Utc::now();
        let id = gen_unique_id();
        Ok(ExperimentRun {
            date,
            id,
            experiment_instance: self.experiment_instance,
            measurements,
        })
    }

    /// Adds a measurement for the output variable with the given `variable_name`
    ///
    /// # panics
    ///
    /// If the current experiment has no output variable with the given name.
    /// If the type of `value` does not match the data type of the variable.
    pub fn add_measurement<S: AsRef<str>>(&self, variable_name: S, value: GenericValue) {
        let variable = self
            .experiment_instance
            .experiment_version()
            .output_variables()
            .iter()
            .find(|v| v.name() == variable_name.as_ref())
            .expect(
                format!(
                    "Variable with name {} not found in output variables of experiment",
                    variable_name.as_ref()
                )
                .as_str(),
            );

        let variable_value = VariableValue::from_variable(variable, value);
        let mut measurements = self.measurements.lock().expect("Lock was poisoned");
        measurements.push(variable_value);
    }
}
