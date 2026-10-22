use std::collections::HashMap;
use std::env;
use std::path::Path;
use std::process::Command;
use std::sync::Mutex;

const GIT_ENV_SSH: &str = "GIT_SSH_COMMAND";

#[cfg(unix)]
const SSH_PERSIST_CMD: &str =
    "ssh -o 'ControlMaster auto' -o 'ControlPath /tmp/.assistant-vault-session--%r@%h:%p' -o 'ControlPersist 1h' -o 'ConnectTimeout 10'";

#[cfg(unix)]
const SSH_PERSIST_HOST_WHITELIST: [&str; 2] = ["github.com", "gitlab.com"];

lazy_static::lazy_static! {
    static ref SSH_PERSIST_GUESS_CACHE: Mutex<HashMap<std::path::PathBuf, bool>> = Mutex::new(HashMap::new());
}

pub fn configure_ssh_persist(cmd: &mut Command) {
    #[cfg(unix)]
    {
        cmd.env(GIT_ENV_SSH, SSH_PERSIST_CMD);
    }
    #[cfg(not(unix))]
    {
        let _ = cmd;
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

        let supported = ssh_uris.iter().all(|uri| match ssh_uri_host(uri) {
            Some(host) => SSH_PERSIST_HOST_WHITELIST.contains(&host.to_lowercase().as_str()),
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

    let before_slash = uri.splitn(2, '/').next().unwrap();
    let after_at = before_slash.splitn(2, '@').last().unwrap();
    let before_colon = after_at.splitn(2, ':').next().unwrap();
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
