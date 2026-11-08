#[cfg(unix)]
use std::collections::HashMap;
use std::env;
use std::path::Path;
use std::process::Command;
#[cfg(unix)]
use std::sync::{LazyLock, Mutex};

const GIT_ENV_SSH: &str = "GIT_SSH_COMMAND";

#[cfg(unix)]
const SSH_PERSIST_CMD: &str =
    "ssh -o 'ControlMaster auto' -o 'ControlPath /tmp/.assistant-vault-session--%r@%h:%p' -o 'ControlPersist 1h' -o 'ConnectTimeout 10' -o 'ServerAliveInterval 15' -o 'ServerAliveCountMax 4'";

const SSH_TIMEOUT_CMD: &str =
    "ssh -o ConnectTimeout=10 -o ServerAliveInterval=15 -o ServerAliveCountMax=4";

const DEFAULT_SSH_PERSIST_HOSTS: [&str; 2] = ["github.com", "gitlab.com"];

#[cfg(unix)]
static SSH_PERSIST_HOSTS: LazyLock<Mutex<Vec<String>>> = LazyLock::new(|| Mutex::new(Vec::new()));

#[cfg(unix)]
static SSH_PERSIST_GUESS_CACHE: LazyLock<Mutex<HashMap<std::path::PathBuf, bool>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn normalize_hosts(hosts: &[String]) -> Vec<String> {
    hosts
        .iter()
        .map(|host| host.trim().to_lowercase())
        .filter(|host| !host.is_empty())
        .collect()
}

pub fn resolve_persist_hosts(configured: &[String]) -> Vec<String> {
    let normalized = normalize_hosts(configured);
    if normalized.is_empty() {
        DEFAULT_SSH_PERSIST_HOSTS
            .iter()
            .map(|host| host.to_string())
            .collect()
    } else {
        normalized
    }
}

pub fn set_ssh_persist_hosts(hosts: Vec<String>) {
    #[cfg(unix)]
    {
        if let Ok(mut guard) = SSH_PERSIST_HOSTS.lock() {
            *guard = normalize_hosts(&hosts);
        }
        if let Ok(mut guard) = SSH_PERSIST_GUESS_CACHE.lock() {
            guard.clear();
        }
    }
    #[cfg(not(unix))]
    {
        let _ = hosts;
    }
}

#[cfg(unix)]
fn persist_hosts() -> Vec<String> {
    let configured = SSH_PERSIST_HOSTS
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or_default();
    resolve_persist_hosts(&configured)
}

pub fn configure_ssh_persist(cmd: &mut Command, repo: &Path) {
    #[cfg(unix)]
    {
        let command = if guess_ssh_persist_support(repo) {
            SSH_PERSIST_CMD
        } else {
            SSH_TIMEOUT_CMD
        };
        cmd.env(GIT_ENV_SSH, command);
    }
    #[cfg(windows)]
    {
        let _ = repo;
        cmd.env(GIT_ENV_SSH, SSH_TIMEOUT_CMD);
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (cmd, repo);
    }
}

pub fn guess_ssh_persist_support(repo: &Path) -> bool {
    if !cfg!(unix) {
        return false;
    }

    if env::var_os(GIT_ENV_SSH).is_some() {
        return false;
    }

    #[cfg(unix)]
    {
        if let Ok(guard) = SSH_PERSIST_GUESS_CACHE.lock() {
            if let Some(supported) = guard.get(repo) {
                return *supported;
            }
        }

        let remotes = match crate::git_sync::git::git_remote(repo) {
            Ok(r) if r.is_empty() => return false,
            Ok(r) => r,
            Err(_) => return false,
        };

        let ssh_uris: Vec<_> = remotes
            .iter()
            .filter_map(|remote| crate::git_sync::git::git_remote_get_url(repo, remote).ok())
            .filter(|uri| !remote_is_http(uri))
            .collect();

        let hosts = persist_hosts();
        let supported = ssh_uris.iter().all(|uri| match ssh_uri_host(uri) {
            Some(host) => hosts.contains(&host.to_lowercase()),
            None => false,
        });

        if let Ok(mut guard) = SSH_PERSIST_GUESS_CACHE.lock() {
            guard.insert(repo.to_path_buf(), supported);
        }

        supported
    }

    #[cfg(not(unix))]
    {
        let _ = repo;
        false
    }
}

#[allow(dead_code)]
fn remote_is_http(url: &str) -> bool {
    let url = url.trim();
    url.starts_with("http://") || url.starts_with("https://")
}

#[allow(dead_code)]
fn ssh_uri_host(uri: &str) -> Option<&str> {
    if remote_is_http(uri) {
        return None;
    }

    let mut uri = uri;

    if let Some(stripped) = uri.strip_prefix("ssh://") {
        uri = stripped;
    }

    let before_slash = uri.split('/').next().unwrap();
    let after_at = before_slash.rsplit('@').next().unwrap();
    let before_colon = after_at.split(':').next().unwrap();
    let host = before_colon.trim();

    if host.len() >= 3 {
        Some(host)
    } else {
        None
    }
}

#[cfg(unix)]
pub fn kill_ssh_sessions() {
    use std::fs;

    let dir = match fs::read_dir("/tmp") {
        Ok(d) => d,
        Err(_) => return,
    };

    let session_files: Vec<_> = dir
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_socket()).unwrap_or(false))
        .filter(|e| {
            e.file_name()
                .to_str()
                .map(|n| n.starts_with(".assistant-vault-session--"))
                .unwrap_or(false)
        })
        .map(|e| e.path())
        .collect();

    for path in session_files {
        log::debug!("Found SSH session file: {:?}", path);
    }
}

#[cfg(not(unix))]
pub fn kill_ssh_sessions() {}
