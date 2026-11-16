use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};

/// stderr 单行截断长度：日志只留可诊断前缀，不整段回放。
const STDERR_LINE_LIMIT: usize = 200;

/// 把 server 的 stderr 收敛成日志。
///
/// 只记 `debug` 且逐行截断：stderr 内容不受我们控制，无法可靠过滤密钥，
/// 因此不做整段回放，也不拼进协议日志。
pub fn spawn_stderr_logger<R>(stderr: R, server_id: String) -> tokio::task::JoinHandle<()>
where
    R: AsyncRead + Send + Unpin + 'static,
{
    tokio::spawn(async move {
        let mut reader = BufReader::new(stderr);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match reader.read_until(b'\n', &mut buf).await {
                Ok(0) => break,
                Ok(_) => {
                    let line = String::from_utf8_lossy(&buf);
                    let line: String = line.trim_end().chars().take(STDERR_LINE_LIMIT).collect();
                    log::debug!("mcp[{server_id}] stderr: {line}");
                }
                Err(_) => break,
            }
        }
    })
}
