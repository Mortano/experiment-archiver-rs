use std::{collections::HashSet, sync::Once};

use anyhow::{anyhow, Result};
use exar::{
    database::db_connection,
    experiment::ExperimentVersion,
    variable::{DataType, GenericValue, Variable},
};
use rand::{distributions::Alphanumeric, thread_rng, Rng};

static INIT: Once = Once::new();

fn setup() {
    INIT.call_once(|| {
        // Important to set env var BEFORE we initiate the DB connection, as this fetches the env var
        std::env::set_var("PSQL_DBSCHEMA", SCHEMA);
        let db = db_connection();
        const SCHEMA: &str = "integration";
        db.init_schema(SCHEMA)
            .expect("Failed to setup test database");
    });
}

fn random_string(length: usize) -> String {
    let mut rng = thread_rng();
    (0..length)
        .map(|_| rng.sample(Alphanumeric) as char)
        .collect()
}

fn default_input_variables() -> HashSet<Variable> {
    [
        Variable::new(random_string(16), random_string(32), DataType::Label),
        Variable::new(random_string(16), random_string(32), DataType::Text),
    ]
    .into_iter()
    .collect()
}

fn alternative_input_variables() -> HashSet<Variable> {
    [Variable::new(
        random_string(17),
        random_string(32),
        DataType::Label,
    )]
    .into_iter()
    .collect()
}

fn default_output_variables() -> HashSet<Variable> {
    [
        Variable::new(random_string(16), random_string(32), DataType::Number),
        Variable::new(
            random_string(16),
            random_string(32),
            DataType::Unit("ms".to_string()),
        ),
    ]
    .into_iter()
    .collect()
}

fn alternative_output_variables() -> HashSet<Variable> {
    [Variable::new(
        random_string(17),
        random_string(32),
        DataType::Number,
    )]
    .into_iter()
    .collect()
}

#[test]
fn insert_experiment() -> Result<()> {
    setup();

    let input_variables = default_input_variables();
    let output_variables = default_output_variables();
    let experiment_version = ExperimentVersion::get_current_version(
        random_string(16),
        random_string(32),
        [random_string(8), random_string(8)].into_iter().collect(),
        input_variables,
        output_variables,
    )?;

    let db = db_connection();
    let all_experiments = db.fetch_experiments()?;
    assert!(all_experiments.contains(&experiment_version.name().to_string()));

    let specific_version = db.fetch_specific_experiment_version(
        experiment_version.name(),
        experiment_version.version(),
    )?;
    assert_eq!(specific_version, Some(experiment_version));

    Ok(())
}

#[test]
fn experiment_versioning() -> Result<()> {
    setup();

    let input_variables = default_input_variables();
    let output_variables = default_output_variables();
    let experiment_version = ExperimentVersion::get_current_version(
        random_string(16),
        random_string(32),
        [random_string(8), random_string(8)].into_iter().collect(),
        input_variables,
        output_variables,
    )?;

    // Change parameters (description, researchers, input variables, output variables) and every time, a new experiment version
    // should be created
    let different_description_ex = ExperimentVersion::get_current_version(
        experiment_version.name().to_string(),
        random_string(33),
        experiment_version.researchers().clone(),
        experiment_version.input_variables().clone(),
        experiment_version.output_variables().clone(),
    )?;
    let different_researchers_ex = ExperimentVersion::get_current_version(
        experiment_version.name().to_string(),
        experiment_version.description().to_string(),
        [random_string(13)].into_iter().collect(),
        experiment_version.input_variables().clone(),
        experiment_version.output_variables().clone(),
    )?;
    let different_input_variables_ex = ExperimentVersion::get_current_version(
        experiment_version.name().to_string(),
        experiment_version.description().to_string(),
        experiment_version.researchers().clone(),
        alternative_input_variables(),
        experiment_version.output_variables().clone(),
    )?;
    let different_output_variables_ex = ExperimentVersion::get_current_version(
        experiment_version.name().to_string(),
        experiment_version.description().to_string(),
        experiment_version.researchers().clone(),
        experiment_version.input_variables().clone(),
        alternative_output_variables(),
    )?;

    let db = db_connection();
    let experiment_versions =
        db.fetch_all_experiment_versions_by_name(experiment_version.name())?;

    assert_eq!(5, experiment_versions.len());
    assert!(experiment_versions.contains(&experiment_version));
    assert!(experiment_versions.contains(&different_description_ex));
    assert!(experiment_versions.contains(&different_researchers_ex));
    assert!(experiment_versions.contains(&different_input_variables_ex));
    assert!(experiment_versions.contains(&different_output_variables_ex));

    Ok(())
}

#[test]
fn experiment_instance() -> Result<()> {
    setup();

    let input_variables = default_input_variables();
    let output_variables = default_output_variables();
    let experiment_version = ExperimentVersion::get_current_version(
        random_string(16),
        random_string(32),
        [random_string(8), random_string(8)].into_iter().collect(),
        input_variables.clone(),
        output_variables.clone(),
    )?;

    let fixed_variables = input_variables
        .iter()
        .map(|var| (var.name(), GenericValue::String(random_string(4))));
    let instance = experiment_version.make_instance(fixed_variables)?;

    let db = db_connection();
    let instance_from_db = db
        .fetch_instance_from_id(instance.id(), &experiment_version)?
        .expect("Specific experiment instance not found in database");
    assert_eq!(instance.id(), instance_from_db.id());
    assert_eq!(
        instance.experiment_version(),
        instance_from_db.experiment_version()
    );
    assert_eq!(
        instance.input_variable_values().len(),
        instance_from_db.input_variable_values().len()
    );
    for expected_var in instance.input_variable_values() {
        instance_from_db
            .input_variable_values()
            .iter()
            .find(|v| *v == expected_var)
            .ok_or_else(|| {
                anyhow!(
                    "Missing value {expected_var:?} in ExperimentInstance fetched from database"
                )
            })?;
    }

    Ok(())
}

#[test]
fn run_experiment() -> Result<()> {
    setup();

    let input_variables = default_input_variables();
    let output_variables = default_output_variables();
    let experiment_version = ExperimentVersion::get_current_version(
        random_string(16),
        random_string(32),
        [random_string(8), random_string(8)].into_iter().collect(),
        input_variables.clone(),
        output_variables.clone(),
    )?;

    let fixed_variables = input_variables
        .iter()
        .map(|var| (var.name(), GenericValue::String(random_string(4))));
    let instance = experiment_version.make_instance(fixed_variables)?;

    let run = instance.run(|context| -> Result<()> {
        for out_var in &output_variables {
            context.add_measurement(out_var.name(), GenericValue::Numeric(42.0));
        }
        Ok(())
    })?;

    let db = db_connection();
    let mut runs = db.fetch_all_runs_of_instance(&instance)?;

    assert_eq!(1, runs.len());
    let run_from_db = runs.remove(0);
    assert_eq!(run_from_db.id(), run.id());
    assert_eq!(run_from_db.date(), run.date());
    assert_eq!(
        run_from_db.experiment_instance().id(),
        run.experiment_instance().id()
    );
    // All measured values should be 42.0
    assert!(run_from_db
        .measurements()
        .iter()
        .all(|val| *val.value() == GenericValue::Numeric(42.0)));

    Ok(())
}

#[test]
fn delete_experiment() -> Result<()> {
    setup();

    // Create an experiment, experiment instance, create a run for it, then delete the experiment and check
    // that the experiment, all versions, all instances, and all runs are gone form the DB!
    let input_variables = default_input_variables();
    let output_variables = default_output_variables();
    let experiment_version = ExperimentVersion::get_current_version(
        random_string(16),
        random_string(32),
        [random_string(8), random_string(8)].into_iter().collect(),
        input_variables.clone(),
        output_variables.clone(),
    )?;

    let fixed_variables = input_variables
        .iter()
        .map(|var| (var.name(), GenericValue::String(random_string(4))));
    let instance = experiment_version.make_instance(fixed_variables)?;

    instance.run(|context| -> Result<()> {
        for out_var in &output_variables {
            context.add_measurement(out_var.name(), GenericValue::Numeric(42.0));
        }
        Ok(())
    })?;

    let db = db_connection();
    db.delete_experiment(experiment_version.name())?;

    let runs = db.fetch_all_runs_of_instance(&instance)?;
    assert!(runs.is_empty());

    let instances = db.fetch_all_instances_of_experiment_version(&experiment_version)?;
    assert!(instances.is_empty());

    let versions = db.fetch_all_experiment_versions_by_name(experiment_version.name())?;
    assert!(versions.is_empty());

    let all_experiments = db.fetch_experiments()?;
    assert!(all_experiments
        .iter()
        .find(|name| *name == experiment_version.name())
        .is_none());

    Ok(())
}
