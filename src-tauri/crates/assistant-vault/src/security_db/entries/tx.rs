use rusqlite::Connection;

use assistant_crypto::vault_crypto::types::MasterKey;

use crate::security_db::SecurityDbState;
use crate::types::VaultError;

impl SecurityDbState {
    pub(crate) fn with_transaction<T, F>(&self, f: F) -> Result<T, VaultError>
    where
        F: FnOnce(&Connection, &MasterKey) -> Result<T, VaultError>,
    {
        let mk_guard = self.master_key.lock().unwrap_or_else(|e| e.into_inner());
        let mk = mk_guard.as_ref().ok_or(VaultError::Locked)?;
        let mut conn_guard = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let conn = conn_guard.as_mut().ok_or(VaultError::Locked)?;
        let tx = conn
            .transaction()
            .map_err(|e| VaultError::Database(e.to_string()))?;
        let result = f(&tx, mk)?;
        tx.commit()
            .map_err(|e| VaultError::Database(e.to_string()))?;
        Ok(result)
    }
}
