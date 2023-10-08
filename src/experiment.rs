use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
    time::SystemTime,
};

use crate::{connect, gen_unique_id, Measurement, RawRun, Run, Variable, VariableTemplate};

use anyhow::{anyhow, bail, Context, Result};
use log::info;
use postgres::{Client, GenericClient};
use tabled::builder::Builder;

/// Context for memorizing variable values while running an experiment. This object is thread-safe
/// so that values can be added safely from multi-threaded code!
pub struct RunContext<'a> {
    experiment: &'a Experiment,
    variable_values: Mutex<HashMap<&'a Variable, String>>,
}

impl<'a> RunContext<'a> {
    pub(crate) fn from_experiment(experiment: &'a Experiment) -> Self {
        Self {
            experiment,
            variable_values: Default::default(),
        }
    }

    /// Adds a value for the variable with the given `variable_name`. You can add anything as a measurement value that is
    /// convertible to strings since the database itself stores all measurement values as strings internally
    ///
    /// # panics
    ///
    /// If the current experiment has no variable with the given name
    pub fn add_value_by_name<S: AsRef<str>, V: ToString>(&self, variable_name: S, value: V) {
        let variable = self
            .experiment
            .variables()
            .find(|v| v.template().name() == variable_name.as_ref())
            .expect("No variable with the given name found in the current experiment!");

        let mut values = self.variable_values.lock().expect("Lock was poisoned");
        values.insert(variable, value.to_string());
    }
}

/// Experiment definition after insertion into the DB or fetching from the DB
#[derive(PartialEq, Eq, Debug)]
pub struct Experiment {
    id: String,
    name: String,
    description: String,
    researcher: String,
    required_variables: HashSet<Variable>,
    autolog_runs: bool,
}

impl Experiment {
    /// Creates a new Experiment with the given parameters. This will insert the experiment into the database if the `name`
    /// is a new unique name, otherwise it will fetch the corresponding experiment from the database. If `name` exists but
    /// the other parameters do not match, this function will return an error. If you want to override an existing experiment,
    /// call `Experiment::override_existing` instead!
    pub fn new(
        name: String,
        description: String,
        researcher: String,
        required_variables: HashSet<VariableTemplate>,
    ) -> Result<Self> {
        let mut db_client =
            crate::postgres::connect().context("Could not connect to postgres DB")?;
        if let Some(experiment) = Self::get_experiment_from_db_by_name(&name, &mut db_client)
            .context("Failed to check database for the existence of this experiment")?
        {
            // Known experiment. check that description and researcher still match, otherwise raise an error!
            if experiment.description != description {
                bail!("Experiment data does not match data of known experiment in the DB! Expected description string {} but got description string {}. Either give the experiment a new unique name, or call `Experiment::override_existing` if you want to replace the experiment data with new data!", description, experiment.description);
            }

            if experiment.researcher != researcher {
                bail!("Experiment data does not match data of known experiment in the DB! Expected researcher(s) {} but got researcher(s) {}. Either give the experiment a new unique name, or call `Experiment::override_existing` if you want to replace the experiment data with new data!", researcher, experiment.researcher);
            }

            let diff_variables = experiment
                .required_variables
                .iter()
                .filter(|variable| {
                    !required_variables.iter().any(|required_variable| {
                        required_variable.name() == variable.template().name()
                    })
                })
                .collect::<Vec<_>>();
            if experiment.required_variables.len() != required_variables.len()
                || !diff_variables.is_empty()
            {
                bail!("Experiment data does not match data of known experiment in the DB! Found unexpected variables {:#?}. If this is a new experiment, either give it a unique name, or call `Experiment::override_existing` if you want to replace the experiment data with new data!", diff_variables);
            }

            Ok(experiment)
        } else {
            // New experiment!
            let mut transaction = db_client
                .transaction()
                .context("Can't start database transaction")?;
            let variables = required_variables
                .into_iter()
                .map(|variable| -> Result<Variable> {
                    // Check if we have a matching variable in the DB!
                    let matching_variable = variable
                        .fetch_from_db(&mut transaction)
                        .context("Failed to check database for existing variable")?;
                    if let Some(variable) = matching_variable {
                        Ok(variable)
                    } else {
                        variable.insert_into_db(&mut transaction)
                    }
                })
                .collect::<Result<HashSet<_>, _>>()
                .context("Inserting variables into database failed")?;
            let experiment_id = Self::insert_new_experiment_into_db(
                &name,
                &description,
                &researcher,
                &variables,
                &mut transaction,
            )
            .context("Failed to insert new experiment into database")?;
            transaction
                .commit()
                .context("Failed to commit transaction for new experiment and variables")?;

            Ok(Self {
                description,
                id: experiment_id,
                name,
                required_variables: variables,
                researcher,
                autolog_runs: false,
            })
        }
    }

    /// Overrides an existing experiment in the database with new values
    pub fn override_existing(
        _name: String,
        _description: String,
        _researcher: String,
        _required_variables: HashSet<VariableTemplate>,
    ) -> Result<()> {
        // TODO Instead of overriding, we could add support for versioning of experiments
        todo!()
    }

    /// Tries to fetch the experiment with the given name from the database
    pub fn from_name(name: &str) -> Result<Option<Self>> {
        let mut db_client =
            crate::postgres::connect().context("Could not connect to postgres DB")?;
        Self::get_experiment_from_db_by_name(name, &mut db_client)
    }

    /// Tries to fetch the experiment for the run with the given ID
    pub fn from_run_id(run_id: &str) -> Result<Option<Self>> {
        let mut db_client =
            crate::postgres::connect().context("Could not connect to postgres DB")?;
        let raw_run =
            RawRun::from_id(run_id, &mut db_client).context("Failed to fetch run from DB")?;
        match raw_run {
            None => Ok(None),
            Some(raw_run) => Self::get_experiment_from_db_by_raw_run(&raw_run, &mut db_client),
        }
    }

    /// Runs this experiment. The code for the experiment is executed as an abstract function passed to this method.
    /// The function itself has to return a set of all variables for this experiment run together with the values for
    /// those variables. Since all measurements are stored in the same DB table, Variable values are stored as strings.
    /// The ID for the experiment run is returned, through this ID information about the run can be queried from the DB
    pub fn run<F: FnOnce(&RunContext) -> Result<()>>(&self, func: F) -> Result<String> {
        let context = RunContext::from_experiment(&self);
        func(&context).context("Experiment function failed")?;

        let measured_variables = context
            .variable_values
            .into_inner()
            .expect("Mutex was poisoned");

        if self.required_variables.len() != measured_variables.len()
            || self
                .required_variables
                .iter()
                .any(|variable| !measured_variables.contains_key(variable))
        {
            bail!("The function passed to `run` must return a value for each required variable in this experiment!");
        }

        let mut db_client =
            crate::postgres::connect().context("Could not connect to postgres DB")?;
        let last_run_number = self
            .get_current_run_number_from_db(&mut db_client)
            .context("Can't get run number of previous run of this experiment")?
            .unwrap_or(0);

        // Insert a new run and one measurement for each variable
        let mut transaction = db_client.transaction().context("Can't start transaction")?;
        let run_id = self
            .insert_run(last_run_number + 1, &mut transaction)
            .context("Failed to insert new experiment run into the database")?;
        for (variable, value) in &measured_variables {
            self.insert_measurement(&variable, &run_id, value.clone(), &mut transaction)
                .context("Failed to insert new measurement")?;
        }
        transaction
            .commit()
            .context("Failed to commit transaction for inserting result of experiment run")?;

        if self.autolog_runs {
            Self::log_run(&measured_variables, last_run_number + 1);
        }

        Ok(run_id)
    }

    /// Fetch all measurements for the given run of this experiment from the DB
    pub fn measurements_for_run(&self, run_id: &str) -> Result<Vec<Measurement>> {
        let mut client = connect().context("Failed to connect to DB")?;

        let run_number_row = client
            .query(
                "SELECT runnumber FROM experiment_runs WHERE id = $1",
                &[&run_id],
            )
            .context("Failed to execute query")?;
        if run_number_row.len() != 1 {
            bail!(
                "Failed to get (unique) run from DB. Expected 1 run, but got {}",
                run_number_row.len()
            );
        }
        let run_number: i32 = run_number_row[0].get(0);

        // There must be one measurement for each of the variables of this experiment!
        self.variables()
            .map(|variable| {
                Measurement::fetch_by_run_and_variable(run_id, run_number, variable, &mut client)
            })
            .collect()
    }

    /// Fetch the data for all runs of this experiments
    pub fn all_runs(&self) -> Result<Vec<Run<'_>>> {
        let mut client = connect().context("Failed to connect to DB")?;

        let all_measurements_for_runs = client
            .query(
                "SELECT experiment_runs.id, value, measurements.timestamp, runnumber, variableid FROM measurements INNER JOIN experiment_runs ON measurements.runid = experiment_runs.id WHERE experiment_runs.experimentid = $1",
                &[&self.id],
            )
            .context("Failed to execute query")?;

        let mut measurements_per_run: HashMap<String, Vec<Measurement<'_>>> = Default::default();

        for row in all_measurements_for_runs {
            let run_id: &str = row.get("id");
            let variable_id: &str = row.get("variableid");
            let value: String = row.get("value");
            let timestamp: SystemTime = row.get("timestamp");
            let run_number: i32 = row.get("runnumber");

            let variable = self
                .required_variables
                .iter()
                .find(|variable| variable.id() == variable_id)
                .ok_or(anyhow!("No matching variable found"))?;

            let measurement = Measurement::new(variable, value, timestamp, run_number);

            if let Some(measurements) = measurements_per_run.get_mut(run_id) {
                measurements.push(measurement);
            } else {
                measurements_per_run.insert(run_id.to_string(), vec![measurement]);
            }
        }

        let mut runs: Vec<Run<'_>> = measurements_per_run
            .into_iter()
            .map(|(run_id, measurements)| {
                Run::new(run_id, measurements[0].run_number() as usize, measurements)
            })
            .collect();
        runs.sort_by(|a, b| a.run_number().cmp(&b.run_number()));

        Ok(runs)
    }

    pub fn run_from_id(&self, run_id: &str) -> Result<Option<Run<'_>>> {
        let mut client = connect().context("Failed to connect to DB")?;
        let raw_run =
            RawRun::from_id(run_id, &mut client).context("Failed to fetch run from DB")?;
        match raw_run {
            None => Ok(None),
            Some(raw_run) => {
                let run =
                    Run::from_raw_run(&raw_run, &self).context("Failed to fetch run from DB")?;
                Ok(Some(run))
            }
        }
    }

    /// Returns the name of this experiment
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the unique ID of this experiment
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the description of this experiment
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Returns the associated researcher for this experiment
    pub fn researcher(&self) -> &str {
        &self.researcher
    }

    /// Returns the set of variables that this experiment requires
    pub fn variables(&self) -> impl Iterator<Item = &Variable> {
        self.required_variables.iter()
    }

    /// Set the autologging feature to active or inactive. If active, every experiment run will be logged
    /// using the `log` crate. By default, autologging is disabled
    pub fn set_autolog_runs(&mut self, autolog_runs: bool) {
        self.autolog_runs = autolog_runs;
    }

    /// Deletes this experiment and all associated data from the database. This function is not undoable, so
    /// be very careful when calling it!
    pub fn delete_from_database(self) -> Result<()> {
        let mut client = connect().context("Failed to connect to DB")?;

        let mut transaction = client
            .transaction()
            .context("Failed to begin transaction")?;

        // delete all measurements for the experiment
        transaction
            .execute(
                "DELETE FROM measurements WHERE experimentid = $1;",
                &[&self.id],
            )
            .context("Failed to delete measurements")?;

        // delete all runs for the experiment
        transaction
            .execute(
                "DELETE FROM experiment_runs WHERE experimentid = $1;",
                &[&self.id],
            )
            .context("Failed to delete experiment runs")?;

        // delete all experiment_variables entries
        transaction
            .execute(
                "DELETE FROM experiment_variables WHERE experiment_id = $1;",
                &[&self.id],
            )
            .context("Failed to delete experiment variables")?;

        // Delete the experiment itself
        transaction
            .execute("DELETE FROM experiments WHERE id = $1;", &[&self.id])
            .context("Failed to delete experiment")?;

        transaction
            .commit()
            .context("Failed to commit transaction")?;

        Ok(())
    }

    /// Delete all runs for this experiment that match the run numbers in `run_numbers`. Also deletes associated measurements
    /// for these runs. This function is not undoable, so be very careful when calling it!
    pub fn delete_runs_from_database(
        &self,
        run_numbers: impl Iterator<Item = usize>,
    ) -> Result<()> {
        let mut client = connect().context("Failed to connect to DB")?;

        let matching_runs = run_numbers
            .map(|run_number| {
                RawRun::from_run_number_and_experiment(run_number, &self, &mut client).and_then(
                    |maybe_run| {
                        maybe_run.ok_or(anyhow!("No run found with run number {run_number}"))
                    },
                )
            })
            .collect::<Result<Vec<_>>>()
            .context("Failed to collect matching runs")?;

        let mut transaction = client
            .transaction()
            .context("Failed to begin transaction")?;

        for run in matching_runs {
            run.delete_from_database(&mut transaction)?;
        }

        transaction
            .commit()
            .context("Failed to commit transaction for deleting runs")?;

        Ok(())
    }

    /// Fetches all experiments from the database
    pub fn all() -> Result<Vec<Experiment>> {
        let mut connection = connect().context("Failed to connect to database")?;
        let experiment_ids = connection
            .query("SELECT id FROM experiments", &[])
            .context("Failed to fetch experiment IDs")?;

        experiment_ids
            .into_iter()
            .map(|id| {
                let id = id.get(0);
                Self::get_experiment_from_db_by_id(id, &mut connection).and_then(|maybe_experiment| maybe_experiment.ok_or(anyhow!("Experiment with ID {} not found in database, even though the ID exists", id)))
            })
            .collect()
    }

    /// Queries the database for an experiment with the same name to determine whether this is a new
    /// experiment or a known experiment
    fn get_experiment_from_db_by_name(
        name: &str,
        client: &mut Client,
    ) -> Result<Option<Experiment>> {
        let rows = client
            .query("SELECT * FROM experiments WHERE name = $1", &[&name])
            .context("Failed to execute query")?;

        match rows.len() {
            0 => Ok(None),
            1 => {
                let row = &rows[0];
                let id: String = row.get("id");

                let variables = Self::query_variables_for_experiment(&id, client).context("Failed to query variables for experiment")?;

                Ok(Some(Experiment { id, name: name.to_owned(), description: row.get("description"), researcher: row.get("researcher"), required_variables: variables, autolog_runs: false, }))
            },
            _ => panic!("Found more than one experiment with the same name, but experiment names have to be unique!"),
        }
    }

    fn get_experiment_from_db_by_id<C: GenericClient>(
        id: &str,
        client: &mut C,
    ) -> Result<Option<Experiment>> {
        let rows = client
            .query("SELECT * FROM experiments WHERE id = $1", &[&id])
            .context("Failed to execute query")?;

        match rows.len() {
            0 => Ok(None),
            1 => {
                let row = &rows[0];
                let id: String = row.get("id");

                let variables = Self::query_variables_for_experiment(&id, client)
                    .context("Failed to query variables for experiment")?;

                Ok(Some(Experiment {
                    id,
                    name: row.get("name"),
                    description: row.get("description"),
                    researcher: row.get("researcher"),
                    required_variables: variables,
                    autolog_runs: false,
                }))
            }
            _ => bail!(
                "Unexpected number of experiments for id {id}. Expected 1 result but got {}",
                rows.len()
            ),
        }
    }

    fn get_experiment_from_db_by_raw_run<C: GenericClient>(
        raw_run: &RawRun,
        client: &mut C,
    ) -> Result<Option<Experiment>> {
        Self::get_experiment_from_db_by_id(&raw_run.experiment_id, client)
    }

    fn query_variables_for_experiment<C: GenericClient>(
        experiment_id: &str,
        client: &mut C,
    ) -> Result<HashSet<Variable>> {
        let rows = client
            .query(
                "SELECT variables.*
            FROM experiments
            JOIN experiment_variables ON experiments.id = experiment_variables.experiment_id
            JOIN variables ON experiment_variables.variable_id = variables.id
            WHERE experiments.id = $1;",
                &[&experiment_id],
            )
            .context("Failed to execute query")?;

        rows.iter()
            .map(|row| -> Result<Variable> {
                let variable = row.try_into()?;
                Ok(variable)
            })
            .collect()
    }

    /// Inserts a new experiment into the database and returns the ID for this new experiment
    fn insert_new_experiment_into_db<C: GenericClient>(
        name: &str,
        description: &str,
        researcher: &str,
        variables: &HashSet<Variable>,
        client: &mut C,
    ) -> Result<String> {
        let id = gen_unique_id();
        let changed_rows = client
            .execute(
                "INSERT INTO experiments VALUES ($1, $2, $3, $4)",
                &[&id, &researcher, &name, &description],
            )
            .context("Failed to execute query")?;

        if changed_rows != 1 {
            bail!("Unexpected number of affected rows. Expected 1 but got {changed_rows}");
        }

        for variable in variables {
            Self::insert_experiment_variable_relation(&id, variable.id(), client)
                .context("Failed to insert experiment/variable relation")?;
        }

        Ok(id)
    }

    fn insert_experiment_variable_relation<C: GenericClient>(
        experiment_id: &str,
        variable_id: &str,
        client: &mut C,
    ) -> Result<()> {
        let changed_rows = client
            .execute(
                "INSERT INTO experiment_variables VALUES ($1, $2)",
                &[&experiment_id, &variable_id],
            )
            .context("Failed to execute INSERT statement for experiment_variables table")?;

        if changed_rows != 1 {
            bail!("Unexpected number of affected rows. Expected 1 but got {changed_rows}");
        }

        Ok(())
    }

    /// Returns the number of the last run of this experiment from the DB. If this experiment has never been run,
    /// `None` is returned
    fn get_current_run_number_from_db(&self, client: &mut Client) -> Result<Option<i32>> {
        let results = client.query("SELECT runnumber FROM experiment_runs WHERE experimentid = $1 ORDER BY runnumber DESC LIMIT 1", &[&self.id]).context("Failed to query experiment_runs table")?;

        match results.len() {
            0 => Ok(None),
            1 => {
                let number = results[0].get::<_, i32>(0);
                Ok(Some(number))
            }
            _ => bail!(
                "Unexpected number of results, expected 1 but got {}",
                results.len()
            ),
        }
    }

    /// Inserts a new experiment run into the DB
    fn insert_run<C: GenericClient>(&self, run_number: i32, client: &mut C) -> Result<String> {
        let run_id = gen_unique_id();
        let timestamp = SystemTime::now();

        let changed_rows = client
            .execute(
                "INSERT INTO experiment_runs VALUES ($1, $2, $3, $4)",
                &[&run_number, &self.id, &run_id, &timestamp],
            )
            .context("Failed to execute INSERT statement for table experiment_runs")?;

        if changed_rows != 1 {
            bail!("Unexpected number of affected rows. Expected 1 but got {changed_rows}");
        }

        Ok(run_id)
    }

    fn insert_measurement<C: GenericClient>(
        &self,
        variable: &Variable,
        run_id: &str,
        value: String,
        client: &mut C,
    ) -> Result<String> {
        let id = gen_unique_id();
        let timestamp = SystemTime::now();

        let changed_rows = client
            .execute(
                "INSERT INTO measurements VALUES ($1, $2, $3, $4, $5)",
                &[&self.id, &variable.id(), &run_id, &value, &timestamp],
            )
            .context("Failed to execute INSERT statement for table measurements")?;

        if changed_rows != 1 {
            bail!("Unexpected number of affected rows. Expected 1 but got {changed_rows}");
        }

        Ok(id)
    }

    fn log_run(variables: &HashMap<&Variable, String>, run_number: i32) {
        info!("Run {run_number}:");

        let mut table_builder = Builder::default();
        table_builder.set_header(variables.iter().map(|(var, _)| var.template().name()));
        table_builder.push_record(variables.iter().map(|(_, value)| value));
        let table = table_builder.build();
        info!("{table}");
    }
}
