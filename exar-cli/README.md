# Command line interface for accessing experiment data

`exar-cli` is a command line tool for accessing experiment data archived with `exar`. With it, you can print information about known experiments, experiment versions, experiment instances and runs both to the command line and to CSV files. You can also edit the underlying database to remove runs, experiment instances, specific experiment versions or all data for an experiment. 

## First usage

After installing with `cargo install --path $PATH_TO_EXAR_CLI_ROOT`, upon first execution you will have to configure a default connection to a database containing `exar` data. Currently, only PostgreSQL is supported as database. After configuration, run `exar-cli --help` to get an overview over all supported commands, or `exar-cli $COMMAND --help` to get help for a specific command. To list all known experiments for example, run:
```
exar-cli ls
```