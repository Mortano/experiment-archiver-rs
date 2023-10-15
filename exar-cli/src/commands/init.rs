use anyhow::Result;
use exar::database::db_connection;

pub fn init() -> Result<()> {
    let schema_name = std::env::var("PSQL_DBSCHEMA")?;
    let db = db_connection();
    db.init_schema(&schema_name)
}
