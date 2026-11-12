/// 远端 URL 视为含密材料：任何日志、错误、UI 展示前都去掉 `user:token@`，
/// 只保留 `scheme://host/path`（sync-crypto-spec §2.8.1）。
pub fn redact_url_userinfo(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(index) = rest.find("://") {
        let split = index + 3;
        out.push_str(&rest[..split]);
        let tail = &rest[split..];
        let authority_end = tail
            .find(|c: char| c.is_whitespace() || matches!(c, '/' | '"' | '\''))
            .unwrap_or(tail.len());
        let authority = &tail[..authority_end];
        match authority.rfind('@') {
            Some(at) => out.push_str(&authority[at + 1..]),
            None => out.push_str(authority),
        }
        rest = &tail[authority_end..];
    }
    out.push_str(rest);
    out
}

/// 单个远端 URL 的展示形态；非 URL 文本原样返回。
pub fn redact_remote_url(url: &str) -> String {
    redact_url_userinfo(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_userinfo_from_https_url() {
        let raw = "https://alice:secret@git.example.com/group/repo.git";
        let redacted = redact_remote_url(raw);
        assert_eq!(redacted, "https://git.example.com/group/repo.git");
        assert!(!redacted.contains("alice"));
        assert!(!redacted.contains("secret"));
    }

    #[test]
    fn strips_userinfo_inside_longer_text() {
        let raw = "fatal: unable to access 'https://token@git.example.com/repo.git/': 403";
        let redacted = redact_url_userinfo(raw);
        assert!(!redacted.contains("token"));
        assert!(redacted.contains("https://git.example.com/repo.git/"));
    }

    #[test]
    fn leaves_url_without_userinfo_untouched() {
        let raw = "https://git.example.com/repo.git";
        assert_eq!(redact_remote_url(raw), raw);
    }

    #[test]
    fn leaves_ssh_scp_style_url_untouched() {
        let raw = "git@git.example.com:group/repo.git";
        assert_eq!(redact_remote_url(raw), raw);
    }
}
