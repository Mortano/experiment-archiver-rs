use std::{
    ops::{Deref, Range},
    sync::{Arc, Mutex},
};

use anyhow::Result;
use chrono::{DateTime, Utc};
use lazy_static::lazy_static;

use super::{
    experiment::{ExperimentInstance, ExperimentVersion},
    run::ExperimentRun,
    variable::VariableValue,
};

mod postgres;

/// Abstract database connection for fetching/inserting experiment data
pub trait Database: Sync + Send {
    /// Returns all experiments by their names
    fn fetch_experiments(&self) -> Result<Vec<String>>;
    fn fetch_latest_experiment_version_by_name(
        &self,
        name: &str,
    ) -> Result<Option<ExperimentVersion>>;
    fn fetch_all_experiment_versions_by_name(&self, name: &str) -> Result<Vec<ExperimentVersion>>;
    fn fetch_specific_experiment_version(
        &self,
        name: &str,
        version: &str,
    ) -> Result<Option<ExperimentVersion>>;
    fn fetch_experiment_version_by_id(&self, version_id: &str)
        -> Result<Option<ExperimentVersion>>;
    fn fetch_experiment_version_from_instance_id(
        &self,
        instance_id: &str,
    ) -> Result<Option<ExperimentVersion>>;

    fn fetch_all_instances_of_experiment_version<'a>(
        &self,
        experiment_version: &'a ExperimentVersion,
    ) -> Result<Vec<ExperimentInstance<'a>>>;
    fn fetch_instance_from_id<'a>(
        &self,
        instance_id: &str,
        experiment_version: &'a ExperimentVersion,
    ) -> Result<Option<ExperimentInstance<'a>>>;
    fn fetch_specific_instance<'a>(
        &self,
        experiment_version: &'a ExperimentVersion,
        input_variable_values: &[VariableValue<'a>],
    ) -> Result<Option<ExperimentInstance<'a>>>;
    fn fetch_all_runs_of_instance<'a>(
        &self,
        experiment_instance: &'a ExperimentInstance<'a>,
    ) -> Result<Vec<ExperimentRun<'a>>>;
    fn fetch_runs_in_date_range<'a>(
        &self,
        experiment_instance: &'a ExperimentInstance<'a>,
        date_range: Range<DateTime<Utc>>,
    ) -> Result<Vec<ExperimentRun<'a>>>;

    fn insert_new_experiment(&self, experiment_version: &ExperimentVersion) -> Result<()>;
    fn insert_new_experiment_version(&self, experiment_version: &ExperimentVersion) -> Result<()>;
    fn insert_new_experiment_instance(
        &self,
        experiment_instance: &ExperimentInstance<'_>,
    ) -> Result<()>;
    fn insert_new_run(&self, run: &ExperimentRun<'_>) -> Result<()>;

    fn delete_experiment(&self, name: &str) -> Result<()>;
    fn delete_experiment_version(&self, experiment_version: &ExperimentVersion) -> Result<()>;
    fn delete_experiment_instance(
        &self,
        experiment_instance: &ExperimentInstance<'_>,
    ) -> Result<()>;

    /// Initialize the database schema `schema_name` with the necessary tables and relations for the `exar` format
    fn init_schema(&self, schema_name: &str) -> Result<()>;
}

lazy_static! {
    static ref CONNECTION: Mutex<Arc<dyn Database>> = {
        // By default, we connect to a PostgreSQL database
        let postgres_db = super::database::postgres::PostgresClient::connect().expect("Failed to connect to PostgreSQL");
        Mutex::new(Arc::new(postgres_db))
    };
}

pub fn db_connection() -> Arc<dyn Database> {
    let connection = CONNECTION.lock().expect("Mutex was poisoned");
    connection.deref().clone()
}

pub fn set_default_db_connection<D: Database + 'static>(new_default_connection: D) {
    let mut lock = CONNECTION.lock().expect("Mutex was poisoned");
    *lock = Arc::new(new_default_connection);
}
