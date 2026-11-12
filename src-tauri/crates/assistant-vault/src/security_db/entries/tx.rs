use rusqlite::Transaction;

use crate::security_db::{SecurityDbState, VaultSession};
use crate::types::VaultError;

impl SecurityDbState {
    pub(crate) fn with_transaction<T, F>(&self, f: F) -> Result<T, VaultError>
    where
        F: FnOnce(&Transaction<'_>, &VaultSession) -> Result<T, VaultError>,
    {
        self.with_session(|session| {
            let tx = session
                .conn
                .unchecked_transaction()
                .map_err(|e| VaultError::Database(e.to_string()))?;
            let result = f(&tx, session)?;
            tx.commit()
                .map_err(|e| VaultError::Database(e.to_string()))?;
            Ok(result)
        })
    }
}
