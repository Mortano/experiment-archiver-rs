use std::time::SystemTime;

use crate::{Experiment, Measurement};
use anyhow::{bail, Context, Result};
use postgres::{GenericClient, Row};

/// Raw structure for an experiment run, matching the scheme of the 'experiment_runs' table. This does not know anything
/// about the measurements associated with the run, for that use the `Run` structure
#[derive(Debug)]
pub struct RawRun {
    pub run_id: String,
    pub run_number: usize,
    pub experiment_id: String,
    pub timestamp: SystemTime,
}

impl RawRun {
    /// Try to fetch the run with the given ID from the database
    pub fn from_id<C: GenericClient>(run_id: &str, client: &mut C) -> Result<Option<Self>> {
        let rows = client
            .query("SELECT * FROM experiment_runs WHERE id = $1", &[&run_id])
            .context("Failed to execute query")?;
        match rows.len() {
            0 => Ok(None),
            1 => {
                let run: Self = (&rows[0])
                    .try_into()
                    .context("Failed to convert DB response to RawRun structure")?;
                Ok(Some(run))
            }
            _ => bail!(
                "Unexpected number of runs for id {run_id}. Expected 1 result but got {}",
                rows.len()
            ),
        }
    }
}

impl TryFrom<&'_ Row> for RawRun {
    type Error = anyhow::Error;

    fn try_from(value: &'_ Row) -> std::result::Result<Self, Self::Error> {
        let run_number: i32 = value
            .try_get("runnumber")
            .context("runnumber field not found in row")?;
        let experiment_id = value
            .try_get("experimentid")
            .context("experimentid field not found in row")?;
        let run_id = value.try_get("id").context("id field not found in row")?;
        let timestamp = value
            .try_get("timestamp")
            .context("timestamp field not found in row")?;
        Ok(Self {
            experiment_id,
            run_id,
            run_number: run_number as usize,
            timestamp,
        })
    }
}

#[derive(Debug)]
pub struct Run<'a> {
    run_id: String,
    run_number: usize,
    measurements: Vec<Measurement<'a>>,
}

impl<'a> Run<'a> {
    pub fn new(run_id: String, run_number: usize, measurements: Vec<Measurement<'a>>) -> Self {
        Self {
            run_id,
            run_number,
            measurements,
        }
    }

    pub fn from_raw_run(raw_run: &RawRun, experiment: &'a Experiment) -> Result<Self> {
        let measurements = experiment
            .measurements_for_run(&raw_run.run_id)
            .context("Failed to fetch measurements from DB")?;
        Ok(Self {
            measurements,
            run_id: raw_run.run_id.to_owned(),
            run_number: raw_run.run_number,
        })
    }

    pub fn id(&self) -> &str {
        &self.run_id
    }

    pub fn run_number(&self) -> usize {
        self.run_number
    }

    pub fn measurements(&self) -> &[Measurement<'a>] {
        &self.measurements
    }
}
