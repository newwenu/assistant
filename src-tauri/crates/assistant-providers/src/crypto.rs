// 参考: ARCHITECTURE.md 安全设计 — API Key AES-256 加密存储

pub fn encrypt_api_key(_key: &str, _machine_id: &[u8]) -> Result<Vec<u8>, String> {
    Err("crypto not yet implemented".to_string())
}

pub fn decrypt_api_key(_encrypted: &[u8], _machine_id: &[u8]) -> Result<String, String> {
    Err("crypto not yet implemented".to_string())
}

pub fn get_machine_id() -> Result<Vec<u8>, String> {
    Err("machine id not yet implemented".to_string())
}