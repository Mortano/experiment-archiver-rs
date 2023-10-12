use std::{collections::HashMap, fs::File, io::BufReader, path::PathBuf};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

/// CLI configuration
#[derive(Serialize, Deserialize, Default)]
pub struct Configuration {
    entries: HashMap<String, ConfigEntry>,
}

impl Configuration {
    /// Loads the current configuration from the default config path on the file system. Returns `Ok(None)` if no
    /// config file exists
    pub fn load() -> Result<Option<Self>> {
        let path = Self::default_path().context("Could not get default config path")?;
        if !path.exists() {
            return Ok(None);
        }
        let config: Self =
            serde_yaml::from_reader(BufReader::new(File::open(&path).with_context(|| {
                format!("Could not open default config file {}", path.display())
            })?))
            .with_context(|| format!("Failed to parse default config file {}", path.display()))?;

        Ok(Some(config))
    }

    /// Store this configuration to disk
    pub fn store(&self) -> Result<()> {
        let path = Self::default_path()?;
        let parent_dir = path.parent().ok_or_else(|| {
            anyhow!(
                "Could not get parent directory of config file path {}",
                path.display()
            )
        })?;
        std::fs::create_dir_all(parent_dir).with_context(|| {
            format!(
                "Failed to create directories for config file {}",
                path.display()
            )
        })?;
        let config_yaml =
            serde_yaml::to_string(self).context("Could not convert Configuration to YAML")?;
        std::fs::write(path, config_yaml).context("Failed to write configuration file")
    }

    /// Applies the default configuration entry from this config
    ///
    /// # Panics
    ///
    /// If there is not exactly one `ConfigEntry` where `is_default` is set to `true` in this `Configuration`
    pub fn apply_default_config(&self) {
        self.get_default_config().1.apply();
    }

    pub fn get_default_config(&self) -> (&String, &ConfigEntry) {
        self.entries
            .iter()
            .find(|(_, config)| config.is_default)
            .expect("No default config entry found")
    }

    pub fn add_entry(&mut self, name: String, entry: ConfigEntry) {
        self.entries.insert(name, entry);
    }

    pub fn remove_entry(&mut self, name: &String) -> bool {
        self.entries.remove(name).is_some()
    }

    pub fn entries(&self) -> &HashMap<String, ConfigEntry> {
        &self.entries
    }

    /// Sets the entry with the given `entry_name` as the default entry
    ///
    /// # panics
    ///
    /// If there is already a `ConfigEntry` set as default with a different name from `entry_name`
    pub fn set_as_default(&mut self, entry_name: &String) {
        if let Some((name, _)) = self.entries.iter().find(|(_, entry)| entry.is_default()) {
            panic!("Entry {name} is already the default entry. Please remove this entry first before setting another entry as default");
        }
        self.entries
            .get_mut(entry_name)
            .expect("Entry not found")
            .set_is_default(true);
    }

    /// Returns the default file path of the config file
    pub fn default_path() -> Result<PathBuf> {
        let user_config_dir = dirs::config_dir()
            .context("Could not determine path of current user's configuration directory")?;
        let config_file_path = user_config_dir.join("exar_cli").join("config.yaml");
        Ok(config_file_path)
    }
}

/// Entry in the CLI configuration refering to a single database containing experiment data in the `exar` format
#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigEntry {
    user: String,
    password: String,
    host: String,
    port: String,
    database_name: String,
    is_default: bool,
}

impl ConfigEntry {
    pub fn new(
        user: String,
        password: String,
        host: String,
        port: String,
        database_name: String,
        is_default: bool,
    ) -> Self {
        Self {
            user,
            password,
            host,
            port,
            database_name,
            is_default,
        }
    }

    /// Applies this configuration. This sets the required environment variables for the experiment-archive crate
    pub fn apply(&self) {
        std::env::set_var("PSQL_USER", &self.user);
        std::env::set_var("PSQL_PWD", &self.password);
        std::env::set_var("PSQL_HOST", &self.host);
        std::env::set_var("PSQL_PORT", &self.port);
        std::env::set_var("PSQL_DBNAME", &self.database_name);
    }

    pub fn is_default(&self) -> bool {
        self.is_default
    }

    pub fn set_is_default(&mut self, is_default: bool) {
        self.is_default = is_default;
    }
}
