# `experiment-archiver` - Run experiment code in Rust and store experiment data in a database

This library is meant as a helper for running experiments based on Rust code and gather the experiment data into a central location using a PostgreSQL database. The following types of experiment data can be stored:
- **Experiment metadata**, such as the experiment name, description, involved researchers
- **Variables**, which are the input and output variables that an experiment requires and measures
- **Experiment runs**, which represent one specific run of an experiment. This allows you to run an experiment multiple times and compare different runs
- **Measurements**, which are the actual measurements from a single experiment run

## Usage

To setup a new experiment, create an instance of the `Experiment` type like so:

```Rust
use experiment_archiver::{Experiment, VariableTemplate};

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
)?;
```

To run an experiment, call `Experiment::run` like so:

```Rust
experiment
    .run(|context| {
        // Your code goes here...
        // Track measured variables like so:
        context.add_value_by_name("Dataset", "Dataset 1");
        context.add_value_by_name("Runtime", 123);
    })?;
```

## Database connection

This library requires a PostgreSQL database with a specific schema. The connection to the database can be configured through a set of environment variables:
- `PSQL_USER` for the PostgreSQL user
- `PSQL_PWD` for the PostgreSQL password
- `PSQL_HOST` and `PSQL_PORT` for the host address and port
- `PSQL_DBNAME` for the name of the database with the required schema

This repository contains a dump of the required SQL schema under `test_data/dbschema.sql`.