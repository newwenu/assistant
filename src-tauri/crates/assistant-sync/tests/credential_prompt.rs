#![cfg(feature = "git-tests")]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::process::Command;

use assistant_sync::git_sync::git::{cmd_git_for_test, git_ls_remote};
use assistant_sync::types::SyncError;

struct EnvGuard {
    key: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvGuard {
    fn set(key: &'static str, value: &std::ffi::OsStr) -> Self {
        let previous = std::env::var_os(key);
        std::env::set_var(key, value);
        Self { key, previous }
    }

    fn set_path(key: &'static str, value: &Path) -> Self {
        Self::set(key, value.as_os_str())
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match self.previous.take() {
            Some(value) => std::env::set_var(self.key, value),
            None => std::env::remove_var(self.key),
        }
    }
}

fn spawn_unauthorized_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let body = b"unauthorized";
            let head = format!(
                "HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"vault-test\"\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(body);
        }
    });
    port
}

#[test]
fn ls_remote_fails_fast_instead_of_prompting_for_credentials() {
    let dir = tempfile::tempdir().unwrap();

    let config = dir.path().join("gcm-gitconfig");
    std::fs::write(&config, b"[credential]\n\thelper = manager\n").unwrap();

    let _global = EnvGuard::set_path("GIT_CONFIG_GLOBAL", &config);
    let _nosystem = EnvGuard::set("GIT_CONFIG_NOSYSTEM", std::ffi::OsStr::new("1"));
    let _askpass = EnvGuard::set("GIT_ASKPASS", std::ffi::OsStr::new(""));
    let _ssh_askpass = EnvGuard::set("SSH_ASKPASS", std::ffi::OsStr::new(""));

    let port = spawn_unauthorized_server();
    let url = format!("http://127.0.0.1:{port}/repo.git");

    let started = std::time::Instant::now();
    let err = git_ls_remote(dir.path(), &url).unwrap_err();
    let elapsed = started.elapsed();

    assert!(
        elapsed < std::time::Duration::from_secs(20),
        "git must fail fast rather than block on a credential prompt, took {elapsed:?}"
    );
    assert!(
        matches!(err, SyncError::GitCli(_)),
        "unauthenticated remote should surface as GitCli error, got {err:?}"
    );
}

fn env_of(cmd: &Command, key: &str) -> Option<String> {
    cmd.get_envs()
        .find(|(k, _)| k.to_string_lossy() == key)
        .and_then(|(_, v)| v.map(|v| v.to_string_lossy().to_string()))
}

fn args_of(cmd: &Command) -> Vec<String> {
    cmd.get_args()
        .map(|a| a.to_string_lossy().to_string())
        .collect()
}

#[test]
fn cmd_git_guards_interaction_and_remote_timeouts() {
    let dir = Path::new(".");
    for connects_remote in [true, false] {
        let cmd = cmd_git_for_test(["--version"], dir, connects_remote);
        assert_eq!(env_of(&cmd, "GIT_TERMINAL_PROMPT").as_deref(), Some("0"));
        assert_eq!(env_of(&cmd, "GCM_INTERACTIVE").as_deref(), Some("never"));

        let args = args_of(&cmd);
        let has_timeout = args.iter().any(|a| a == "http.timeout=30");
        assert_eq!(has_timeout, connects_remote, "http.timeout is remote-only");
    }
}
