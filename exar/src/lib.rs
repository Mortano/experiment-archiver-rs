pub mod database;
pub mod experiment;
pub mod run;
pub(crate) mod util;
pub mod variable;
pub(crate) mod version;

const ENV_EXAR_LOCAL: &str = "EXAR_LOCAL";

/// Synchronize all data with the database? If the `EXAR_LOCAL` environment variable is set to `1`, no database
/// connection is established and all data is only handled locally.
pub fn use_database() -> bool {
    let no_db = std::env::var(ENV_EXAR_LOCAL) == Ok("1".to_string());
    !no_db
}
