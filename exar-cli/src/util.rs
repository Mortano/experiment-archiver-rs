use std::io::Write;

use anyhow::{bail, Context, Result};
use exar::variable::{DataType, Variable, VariableValue};
use serde::Serialize;

/// Read a string from the command line by prompting the user with the given `prompt`
pub fn read_string_with_prompt<S: AsRef<str>>(prompt: S) -> Result<String> {
    let mut ret = String::default();
    print!("{}: ", prompt.as_ref());
    std::io::stdout().flush()?;
    std::io::stdin()
        .read_line(&mut ret)
        .context("Failed to read line")?;
    Ok(ret.trim().to_string())
}

/// Like `read_string_with_prompt`, but prompts for a boolean value with `(y/n)` syntax
pub fn read_bool_with_prompt<S: AsRef<str>>(prompt: S) -> Result<bool> {
    let mut choice = String::default();
    print!("{} (y/n): ", prompt.as_ref());
    std::io::stdout().flush()?;
    std::io::stdin()
        .read_line(&mut choice)
        .context("Failed to read line")?;
    match choice.trim() {
        "y" => Ok(true),
        "n" => Ok(false),
        other => bail!("Invalid input {other}"),
    }
}

/// Print an options list using the given `options` to the terminal and return the zero-based index of the
/// user-selected option. Options are printed as a numerical list starting with `(1)`.
pub fn options_list<S1: AsRef<str>, S2: AsRef<str>>(header: S1, options: &[S2]) -> Result<usize> {
    if options.is_empty() {
        bail!("Options list must not be empty");
    }
    println!("{}", header.as_ref());
    for (idx, option) in options.iter().enumerate() {
        println!("({}): {}", idx + 1, option.as_ref());
    }
    std::io::stdout().flush().expect("Failed to flush stdout");
    let mut selection = String::new();
    std::io::stdin()
        .read_line(&mut selection)
        .context("Failed to read line")?;
    let selected_idx = selection.trim().parse::<usize>().with_context(|| {
        format!(
            "Invalid selection {selection}, select a number between 1 and {}",
            options.len()
        )
    })?;
    if selected_idx > options.len() {
        bail!(
            "Index out of range, please select an index between 1 and {}",
            options.len()
        );
    }
    Ok(selected_idx - 1)
}

/// A serializable version of a `VariableValue`
#[derive(Debug, Serialize)]
pub struct SerializableVariableValue {
    pub name: String,
    pub value: String,
}

impl From<&VariableValue<'_>> for SerializableVariableValue {
    fn from(value: &VariableValue<'_>) -> Self {
        Self {
            name: value.variable().name().to_string(),
            value: value.value().to_string(),
        }
    }
}

pub fn print_serializable_as_json<S: Serialize>(serializables: &[S]) -> Result<()> {
    let json = serde_json::to_string(serializables)?;
    print!("{json}");
    Ok(())
}

pub fn print_serializable_as_yaml<S: Serialize>(serializables: &[S]) -> Result<()> {
    let yaml = serde_yaml::to_string(serializables)?;
    print!("{yaml}");
    Ok(())
}

/// Returns a string for displaying in a table that represents the given variable
pub fn variable_to_table_display(variable: &Variable) -> String {
    match variable.data_type() {
        DataType::Unit(unit) => format!("{} ({unit})", variable.name()),
        _ => variable.name().to_string(),
    }
}
