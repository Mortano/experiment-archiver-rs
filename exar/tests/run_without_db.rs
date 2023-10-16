use std::{collections::HashSet, sync::Once};

use anyhow::Result;
use exar::{
    experiment::ExperimentVersion,
    variable::{DataType, GenericValue, Variable},
};
use rand::{distributions::Alphanumeric, thread_rng, Rng};

static INIT: Once = Once::new();

fn setup() {
    INIT.call_once(|| {
        // Set env var to run without database connection
        std::env::set_var("EXAR_LOCAL", "1");
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

    // All measured values should be 42.0
    assert!(run
        .measurements()
        .iter()
        .all(|val| *val.value() == GenericValue::Numeric(42.0)));

    Ok(())
}
