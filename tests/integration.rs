/**
 * Tests are written in a way that assumes no individual test setup/teardown. Instead, all tests assume that
 * a single postgres DB is running, that is setup prior to all tests. Since no test can assume unique ownership
 * of the DB, some test code is written a bit different because other tests might add data to the DB concurrently.
 * To prevent collisions, all test data uses random IDs, the chance of ID collisions will be very low since IDs
 * are 16-character strings, so something like 1 in 62^16
 */
use std::collections::{HashMap, HashSet};

use experiment_archiver::{self, Experiment, VariableTemplate};

use anyhow::{Context, Result};
use rand::{distributions::Alphanumeric, thread_rng, Rng};

fn random_string(length: usize) -> String {
    let mut rng = thread_rng();
    (0..length)
        .map(|_| rng.sample(Alphanumeric) as char)
        .collect()
}

#[test]
fn showcase() -> Result<()> {
    let variables = [
        VariableTemplate::new(
            "Dataset".into(),
            "The dataset used for this experiment".into(),
            "none".into(),
        ),
        VariableTemplate::new(
            "Runtime".into(),
            "The runtime of the experiment".into(),
            "ms".into(),
        ),
    ]
    .into_iter()
    .collect::<HashSet<_>>();

    let experiment = Experiment::new(
        "Performance Test 1".into(),
        "This experiment tests general performance of tool XYZ".into(),
        "Name of the researchers".into(),
        variables,
    )
    .context("Failed to create experiment")?;

    experiment
        .run(|context| {
            // Your code goes here
            // Track measured variables like so:
            context.add_value_by_name("Dataset", "Dataset 1");
            context.add_value_by_name("Runtime", 123);

            Ok(())
        })
        .context("Failed to run experiment")?;

    Ok(())
}

#[test]
fn new_experiment_is_added_to_db() -> Result<()> {
    const NUM_VARIABLES: usize = 4;
    let variables = (0..NUM_VARIABLES)
        .map(|_| {
            VariableTemplate::new(
                random_string(16).into(),
                random_string(32).into(),
                random_string(8).into(),
            )
        })
        .collect();

    let experiment = Experiment::new(
        random_string(16),
        random_string(32),
        random_string(16),
        variables,
    )
    .context("Failed to create new Experiment")?;

    let all_experiments = Experiment::all().context("Failed to fetch experiments from DB")?;
    let matching_experiment = all_experiments.iter().find(|ex| ex.id() == experiment.id());
    assert_eq!(Some(&experiment), matching_experiment);

    Ok(())
}

#[test]
fn add_known_experiment() -> Result<()> {
    const NUM_VARIABLES: usize = 4;
    let variables: HashSet<_> = (0..NUM_VARIABLES)
        .map(|_| {
            VariableTemplate::new(
                random_string(16).into(),
                random_string(32).into(),
                random_string(8).into(),
            )
        })
        .collect();

    let name = random_string(16);
    let description = random_string(32);
    let researcher = random_string(16);

    let experiment1 = Experiment::new(
        name.clone(),
        description.clone(),
        researcher.clone(),
        variables.clone(),
    )
    .context("Failed to create new Experiment")?;

    // Add experiment again, which should yield an `Experiment` object with the same ID as experiment1
    let experiment2 = Experiment::new(name, description, researcher, variables)
        .context("Failed to create new Experiment")?;
    assert_eq!(experiment1, experiment2);

    let all_experiments = Experiment::all().context("Failed to fetch experiments from DB")?;
    assert_eq!(
        1,
        all_experiments
            .iter()
            .filter(|ex| **ex == experiment1)
            .count()
    );

    Ok(())
}

#[test]
fn new_experiment_run() -> Result<()> {
    const NUM_VARIABLES: usize = 4;
    let variables = (0..NUM_VARIABLES)
        .map(|_| {
            VariableTemplate::new(
                random_string(16).into(),
                random_string(32).into(),
                random_string(8).into(),
            )
        })
        .collect();

    let experiment = Experiment::new(
        random_string(16),
        random_string(32),
        random_string(16),
        variables,
    )
    .context("Failed to create new Experiment")?;

    let expected_measurements_for_run: HashMap<_, _> = experiment
        .variables()
        .map(|variable| (variable.clone(), random_string(8)))
        .collect();

    let run_id = experiment
        .run(|context| {
            for (variable, value) in &expected_measurements_for_run {
                context.add_value_by_name(variable.template().name(), &value);
            }

            Ok(())
        })
        .context("Experiment run failed")?;

    let actual_measurements_for_run = experiment
        .measurements_for_run(&run_id)
        .context("Failed to get measurements for experiment run")?;

    assert_eq!(
        expected_measurements_for_run.len(),
        actual_measurements_for_run.len()
    );

    for measurement in actual_measurements_for_run {
        let expected_value = expected_measurements_for_run
            .get(measurement.variable())
            .expect("Unexpected variable");
        assert_eq!(expected_value, measurement.value());
    }

    Ok(())
}

#[test]
fn delete_experiment() -> Result<()> {
    // Create new experiment, add a run, then delete experiment and verify that everything was deleted
    const NUM_VARIABLES: usize = 4;
    let variables: HashSet<_> = (0..NUM_VARIABLES)
        .map(|_| {
            VariableTemplate::new(
                random_string(16).into(),
                random_string(32).into(),
                random_string(8).into(),
            )
        })
        .collect();

    let name = random_string(16);
    let description = random_string(32);
    let researcher = random_string(16);

    let experiment1 = Experiment::new(
        name.clone(),
        description.clone(),
        researcher.clone(),
        variables.clone(),
    )
    .context("Failed to create new Experiment")?;

    let expected_measurements_for_run: HashMap<_, _> = experiment1
        .variables()
        .map(|variable| (variable.clone(), random_string(8)))
        .collect();
    experiment1
        .run(|context| {
            for (variable, value) in &expected_measurements_for_run {
                context.add_value_by_name(variable.template().name(), &value);
            }

            Ok(())
        })
        .context("Experiment run failed")?;

    experiment1
        .delete_from_database()
        .context("Deleting experiment failed")?;

    assert_eq!(None, Experiment::from_name(&name)?);

    // Creating new experiment with same parameters should yield a new experiment with zero runs
    let same_experiment = Experiment::new(
        name.clone(),
        description.clone(),
        researcher.clone(),
        variables.clone(),
    )
    .context("Failed to create Experiment")?;
    let runs = same_experiment
        .all_runs()
        .context("failed to get experiment runs")?;
    assert!(runs.is_empty());

    Ok(())
}

#[test]
fn delete_runs() -> Result<()> {
    const NUM_VARIABLES: usize = 4;
    let variables: HashSet<_> = (0..NUM_VARIABLES)
        .map(|_| {
            VariableTemplate::new(
                random_string(16).into(),
                random_string(32).into(),
                random_string(8).into(),
            )
        })
        .collect();

    let name = random_string(16);
    let description = random_string(32);
    let researcher = random_string(16);

    let experiment1 = Experiment::new(
        name.clone(),
        description.clone(),
        researcher.clone(),
        variables.clone(),
    )
    .context("Failed to create new Experiment")?;

    let expected_measurements_for_run: HashMap<_, _> = experiment1
        .variables()
        .map(|variable| (variable.clone(), random_string(8)))
        .collect();

    // Add a bunch of runs
    const NUM_RUNS: usize = 12;
    for _ in 0..NUM_RUNS {
        experiment1
            .run(|context| {
                for (variable, value) in &expected_measurements_for_run {
                    context.add_value_by_name(variable.template().name(), &value);
                }

                Ok(())
            })
            .context("Experiment run failed")?;
    }

    // Run numbers start at 1. Delete first run and verify that only this run was deleted
    experiment1.delete_runs_from_database(1..2)?;
    let expected_run_numbers = (2..=NUM_RUNS).collect::<Vec<_>>();
    let actual_run_numbers = experiment1
        .all_runs()?
        .into_iter()
        .map(|run| run.run_number())
        .collect::<Vec<_>>();
    assert_eq!(expected_run_numbers, actual_run_numbers);

    // Delete all other runs. No runs should be remaining
    experiment1.delete_runs_from_database(2..=NUM_RUNS)?;
    assert!(experiment1.all_runs()?.is_empty());

    Ok(())
}
