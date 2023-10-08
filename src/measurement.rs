use anyhow::{bail, Context, Result};
use std::time::SystemTime;

use postgres::GenericClient;

use crate::Variable;

/// A single measurement from an experiment and run
#[derive(Debug)]
pub struct Measurement<'a> {
    variable: &'a Variable,
    value: String,
    timestamp: SystemTime,
    run_number: i32,
}

impl<'a> Measurement<'a> {
    pub(crate) fn new(
        variable: &'a Variable,
        value: String,
        timestamp: SystemTime,
        run_number: i32,
    ) -> Self {
        Self {
            variable,
            value,
            timestamp,
            run_number,
        }
    }

    /// Fetch a Measurement from the DB using the given run ID and variable
    pub(crate) fn fetch_by_run_and_variable<C: GenericClient>(
        run_id: &str,
        run_number: i32,
        variable: &'a Variable,
        client: &mut C,
    ) -> Result<Self> {
        let matching_rows = client
            .query(
                "SELECT * FROM measurements WHERE runid = $1 AND variableid = $2",
                &[&run_id, &variable.id()],
            )
            .context("Failed to query for measurements")?;

        if matching_rows.len() != 1 {
            bail!(
                "Failed to get (unique) measurement from DB. Expected 1 measurement, but got {}",
                matching_rows.len()
            );
        }

        let row = &matching_rows[0];

        Ok(Self {
            variable,
            value: row.get("value"),
            timestamp: row.get("timestamp"),
            run_number,
        })
    }

    /// Access the value of this measurement
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Access the timestamp of this measurement
    pub fn timestamp(&self) -> SystemTime {
        self.timestamp
    }

    /// Access the run number of this measurement
    pub fn run_number(&self) -> i32 {
        self.run_number
    }

    /// Access the Variable for this measurement
    pub fn variable(&self) -> &Variable {
        self.variable
    }
}
