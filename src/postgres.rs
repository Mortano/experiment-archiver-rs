use anyhow::{Context, Result};
use postgres::{Client, Config, NoTls};

const ENV_PSQL_USER: &str = "PSQL_USER";
const ENV_PSQL_PWD: &str = "PSQL_PWD";
const ENV_PSQL_HOST: &str = "PSQL_HOST";
const ENV_PSQL_PORT: &str = "PSQL_PORT";
const ENV_PSQL_DBNAME: &str = "PSQL_DBNAME";

/// Returns connection config for postgres DB, fetched from environment variables
pub(crate) fn get_postgres_config() -> Result<Config> {
    let mut config = Client::configure();
    config
        .host(
            std::env::var(ENV_PSQL_HOST)
                .context("Could not get host name for postgres connection")?
                .as_str(),
        )
        .port(
            std::env::var(ENV_PSQL_PORT)
                .context("Could not get port for postgres connection")?
                .as_str()
                .parse()
                .context(format!(
                    "Value of {ENV_PSQL_PORT} must be a valid port number"
                ))?,
        )
        .user(
            std::env::var(ENV_PSQL_USER)
                .context("Could not user host name for postgres connection")?
                .as_str(),
        )
        .password(
            std::env::var(ENV_PSQL_PWD)
                .context("Could not get password for postgres connection")?
                .as_str(),
        )
        .dbname(
            std::env::var(ENV_PSQL_DBNAME)
                .context("Could not get database name for postgres connection")?
                .as_str(),
        );
    Ok(config)
}

/// Connects to the postgres DB and returns a Client
pub(crate) fn connect() -> Result<Client> {
    let config =
        get_postgres_config().context("Can't get connection configuration for postgres DB")?;
    let client = config.connect(NoTls).context(format!(
        "Could not connect to postgres DB with config {:?}",
        config
    ))?;
    Ok(client)
}
