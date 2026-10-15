use std::collections::HashMap;
use std::sync::Mutex;
use tokio_util::sync::CancellationToken;

pub struct ChatTasks(pub Mutex<HashMap<String, CancellationToken>>);

impl ChatTasks {
    pub fn new() -> Self {
        Self(Mutex::new(HashMap::new()))
    }

    fn lock_map(&self) -> std::sync::MutexGuard<'_, HashMap<String, CancellationToken>> {
        match self.0.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                log::warn!("ChatTasks mutex poisoned, recovering");
                poisoned.into_inner()
            }
        }
    }

    pub fn start(&self, conversation_id: String) -> CancellationToken {
        let token = CancellationToken::new();
        let clone = token.clone();
        let mut map = self.lock_map();
        if let Some(old) = map.insert(conversation_id, token) {
            old.cancel();
        }
        clone
    }

    pub fn cancel(&self, conversation_id: &str) {
        let mut map = self.lock_map();
        if let Some(token) = map.remove(conversation_id) {
            token.cancel();
        }
    }

    pub fn finish(&self, conversation_id: &str) {
        let mut map = self.lock_map();
        map.remove(conversation_id);
    }

    pub fn is_running(&self, conversation_id: &str) -> bool {
        let map = self.lock_map();
        map.contains_key(conversation_id)
    }
}
