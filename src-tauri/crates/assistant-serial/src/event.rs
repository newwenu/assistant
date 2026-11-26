use assistant_protocol::{SerialRxBatchDto, SerialSessionDto};

/// 领域事件：只描述"发生了什么"，怎么发由装配层注入的 sink 决定
#[derive(Debug, Clone)]
pub enum SerialEvent {
    Rx(SerialRxBatchDto),
    Status(SerialSessionDto),
}

/// 事件出口：装配层注入（tauri emit / 测试收集器）
pub type EventSink = std::sync::Arc<dyn Fn(SerialEvent) + Send + Sync>;

pub fn noop_sink() -> EventSink {
    std::sync::Arc::new(|_| {})
}
