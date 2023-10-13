use anyhow::Result;
use exar::{
    experiment::ExperimentVersion,
    variable::{DataType, GenericValue, Variable},
};

#[test]
fn showcase() -> Result<()> {
    let input_variables = [
        Variable::new(
            "Variable 1".to_string(),
            "Description of variable 1".to_string(),
            DataType::Label,
        ),
        Variable::new(
            "Variable 2".to_string(),
            "Description of variable 2".to_string(),
            DataType::Text,
        ),
    ]
    .into_iter()
    .collect();

    let output_variables = [
        Variable::new(
            "Out var 1".to_string(),
            "The first output variable".to_string(),
            DataType::Number,
        ),
        Variable::new(
            "Out var 2".to_string(),
            "An output variable with a unit!".to_string(),
            DataType::Unit("m/s".to_string()),
        ),
    ]
    .into_iter()
    .collect();

    let researchers = ["Dr. Re Search".to_string(), "Prof. Guy McData".to_string()]
        .into_iter()
        .collect();

    let experiment = ExperimentVersion::get_current_version(
        "Test experiment 1".to_string(),
        "Description of the test experiment".to_string(),
        researchers,
        input_variables,
        output_variables,
    )?;

    let instance = experiment.make_instance([
        (
            "Variable 1",
            GenericValue::String("Fixed value".to_string()),
        ),
        (
            "Variable 2",
            GenericValue::String("Another fixed value".to_string()),
        ),
    ])?;

    instance.run(|context| -> Result<()> {
        // execute some code...
        println!("running experiment...");
        // Then add measurements
        context.add_measurement("Out var 1", GenericValue::Numeric(42.0));
        context.add_measurement("Out var 2", GenericValue::Numeric(-10.0));
        Ok(())
    })?;

    Ok(())
}