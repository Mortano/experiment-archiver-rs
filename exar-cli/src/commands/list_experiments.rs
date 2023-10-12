use anyhow::{Context, Result};
use exar::database::db_connection;
use tabled::{builder::Builder, settings::Style};

use crate::OutputFormat;

/// List all experiments by name
pub fn list_experiments(output_format: OutputFormat) -> Result<()> {
    let db = db_connection();
    let mut experiment_names = db
        .fetch_experiments()
        .context("Failed to fetch experiments from the database")?;
    experiment_names.sort();

    match output_format {
        OutputFormat::Table => print_experiments_as_table(&experiment_names),
        OutputFormat::CSV => print_experiments_as_csv(&experiment_names),
        OutputFormat::JSON => print_experiments_as_json(&experiment_names),
        OutputFormat::YAML => print_experiments_as_yaml(&experiment_names),
    }
}

fn print_experiments_as_table(experiment_names: &[String]) -> Result<()> {
    let mut table_builder = Builder::default();
    table_builder.set_header(["ID", "Name"]);

    for (index, name) in experiment_names.iter().enumerate() {
        table_builder.push_record([format!("{}", index + 1).as_str(), name.as_str()]);
    }

    let mut table = table_builder.build();

    // let (terminal_width, _) =
    //     termion::terminal_size().context("Can't determine terminal size")?;
    // table.with(Width::wrap(terminal_width as usize));
    // table.with(Modify::new(Rows::new(..)).with(Width::wrap(24)));
    table.with(Style::modern());

    println!("{table}");
    Ok(())
}

fn print_experiments_as_csv(experiment_names: &[String]) -> Result<()> {
    println!("Name");
    for (index, name) in experiment_names.iter().enumerate() {
        if index == experiment_names.len() - 1 {
            print!("{name}");
        } else {
            println!("{name}");
        }
    }
    Ok(())
}

fn print_experiments_as_json(experiment_names: &[String]) -> Result<()> {
    let as_json = serde_json::to_string(experiment_names)
        .context("Could not convert experiment names to JSON")?;
    print!("{as_json}");
    Ok(())
}

fn print_experiments_as_yaml(experiment_names: &[String]) -> Result<()> {
    let as_yaml = serde_yaml::to_string(experiment_names)
        .context("Could not convert experiment names to YAML")?;
    print!("{as_yaml}");
    Ok(())
}
