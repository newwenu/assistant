/// turn 代际 id，由 runtime 自建的 TurnTasks 生成，格式与线字段 attempt_id 一致
/// （`{conversation_id}:{counter}`）。线字段仍叫 `attempt_id`，见 design §3.1。
pub type TurnId = String;

#[derive(Debug, Clone)]
pub struct TurnContext {
    pub conversation_id: String,
    pub turn_id: TurnId,
    /// 本轮助手消息 id，由宿主在建消息时生成，内核只负责写内容与终态。
    pub message_id: String,
    /// 会话内 turn 序号（I46 §2.5）：由宿主取号后透传，内核只搬运不判定。
    /// `None` = 该轮无坐标（存量分支点），落库写 `NULL`。
    pub turn_index: Option<u32>,
    /// 轮内迭代序号，从 1 开始；provider 重试不递增。
    pub iteration: u32,
    /// 本次模型调用的 provider 重试次数，由 adapter 上报，只读。
    pub provider_attempt: u32,
}

/// `iteration` 落库并进 `MessageDto`（I46 D2）；`provider_attempt` 仍只在事件与日志。
/// 理由见 design §3.1 / §3.10。
impl TurnContext {
    pub fn new(
        conversation_id: impl Into<String>,
        turn_id: impl Into<String>,
        message_id: impl Into<String>,
        turn_index: Option<u32>,
    ) -> Self {
        Self {
            conversation_id: conversation_id.into(),
            turn_id: turn_id.into(),
            message_id: message_id.into(),
            turn_index,
            iteration: 1,
            provider_attempt: 0,
        }
    }
}
