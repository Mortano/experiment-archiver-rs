use std::io::Write;

use crate::{
    configuration::{ConfigEntry, Configuration},
    util::{options_list, read_bool_with_prompt, read_string_with_prompt},
};
use anyhow::{bail, Context, Result};
use rpassword::read_password;

/// Run the initial configuration. Prompts the user to input a new configuration, sets this configuration as default
/// and returns it as a pair (name, configuration)
pub fn initial_configuration() -> Result<(String, ConfigEntry)> {
    let config_name = read_string_with_prompt("Enter name of new configuration")?;
    let config = create_new_configuration(true)?;
    Ok((config_name, config))
}

/// Prompts the user to input a new configuration. Will be set as the new default configuration if `as_new_default`
/// is `true`
fn create_new_configuration(as_new_default: bool) -> Result<ConfigEntry> {
    let host = read_string_with_prompt("Enter hostname of PostgreSQL database")?;
    let port = read_string_with_prompt("Enter port of PostgreSQL database")?;
    let user = read_string_with_prompt("Enter username of PostgreSQL database")?;

    print!("Enter password of PostgreSQL database: ");
    std::io::stdout().flush().expect("Failed to flush stdout");
    let password = read_password().context("Failed to read password")?;

    let db_name = read_string_with_prompt("Enter name of database containg `exar` data")?;

    let config = ConfigEntry::new(user, password, host, port, db_name, as_new_default);
    Ok(config)
}

pub fn configure() -> Result<()> {
    let current_config = Configuration::load()
        .context("Failed to load current configuration")?
        .expect("configure() must not be called if there is no current configuration!");

    let selected_option = options_list(
        "What do you want to do?",
        &[
            "Add new configuration",
            "Delete existing configuration",
            "Delete all configurations",
        ],
    )?;
    match selected_option {
        0 => add_new_config(current_config),
        1 => delete_existing_config(current_config),
        2 => delete_all_configs(),
        other => bail!("Invalid index {other}"),
    }
}

fn add_new_config(mut current_config: Configuration) -> Result<()> {
    let config_name = read_string_with_prompt("Enter name of new configuration")?;
    if current_config.entries().contains_key(&config_name) {
        bail!("Configuration with name {config_name} already exists!");
    }

    let mut new_config =
        create_new_configuration(false).context("Could not create new configuration")?;

    let (default_name, _) = current_config.get_default_config();
    let as_new_default = read_bool_with_prompt(format!(
        "Set configuration {} as new default (replacing current default configuration {})?",
        config_name, default_name
    ))?;
    if as_new_default {
        new_config.set_is_default(true);
    }
    current_config.add_entry(config_name, new_config);
    current_config
        .store()
        .context("Failed to save configuration")?;

    Ok(())
}

fn delete_existing_config(mut current_config: Configuration) -> Result<()> {
    let mut existing_names = current_config.entries().keys().cloned().collect::<Vec<_>>();
    existing_names.sort();

    let index_to_delete = options_list("Configuration to delete", &existing_names)?;
    let config_to_delete = current_config
        .entries()
        .get(&existing_names[index_to_delete])
        .unwrap();
    let is_default_config = config_to_delete.is_default();
    current_config.remove_entry(&existing_names[index_to_delete]);

    // If there are multiple config entries and this is the default one, prompt for a new default
    if current_config.entries().len() > 1 && is_default_config {
        existing_names.remove(index_to_delete);
        let new_default_index = options_list("Choose new default configuration", &existing_names)?;
        current_config.set_as_default(&existing_names[new_default_index]);
    }

    current_config
        .store()
        .context("Failed to save configuration")?;

    Ok(())
}

fn delete_all_configs() -> Result<()> {
    let config_path =
        Configuration::default_path().context("Could not get default configuration file path")?;
    std::fs::remove_file(config_path).context("Could not remove config file")?;
    Ok(())
}
