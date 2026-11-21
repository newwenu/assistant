use rusqlite::Connection;

#[allow(dead_code)]
pub fn run_migrations(_conn: &Connection) -> Result<(), String> {
    log::trace!("Migration function disabled — database already migrated manually");
    Ok(())
}
