use std::collections::HashMap;
use std::sync::Mutex;
use tokio_util::sync::CancellationToken;

pub struct ChatTaskEntry {
    pub cancel_token: CancellationToken,
    pub attempt_counter: u32,
}

pub struct ChatTasks(pub Mutex<HashMap<String, ChatTaskEntry>>);

impl ChatTasks {
    pub fn new() -> Self {
        Self(Mutex::new(HashMap::new()))
    }

    fn lock_map(&self) -> std::sync::MutexGuard<'_, HashMap<String, ChatTaskEntry>> {
        match self.0.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                log::warn!("ChatTasks mutex poisoned, recovering");
                poisoned.into_inner()
            }
        }
    }

    pub fn start(&self, conversation_id: String) -> (CancellationToken, String) {
        let token = CancellationToken::new();
        let clone = token.clone();
        let mut map = self.lock_map();
        if let Some(old) = map.get_mut(&conversation_id) {
            old.cancel_token.cancel();
            old.attempt_counter += 1;
            old.cancel_token = token;
            let attempt_id = format!("{}:{}", conversation_id, old.attempt_counter);
            (clone, attempt_id)
        } else {
            let entry = ChatTaskEntry {
                cancel_token: token,
                attempt_counter: 1,
            };
            map.insert(conversation_id.clone(), entry);
            (clone, format!("{}:1", conversation_id))
        }
    }

    pub fn cancel(&self, conversation_id: &str) {
        let mut map = self.lock_map();
        if let Some(entry) = map.remove(conversation_id) {
            entry.cancel_token.cancel();
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
