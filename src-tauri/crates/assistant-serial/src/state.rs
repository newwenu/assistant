use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;

use assistant_protocol::{
    SerialConfigDto, SerialDataBitsDto, SerialDebugErrorDto, SerialFlowControlDto,
    SerialOpenInputDto, SerialParityDto, SerialSessionDto, SerialStopBitsDto,
};

use crate::backend::PortHandle;

pub(crate) type HandleSlot = Arc<Mutex<Option<Box<dyn PortHandle>>>>;

/// 已生效参数：与 `SerialConfigDto` 同构，另带打开用的端口名
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerialConfig {
    pub port_name: String,
    pub baud_rate: u32,
    pub data_bits: SerialDataBitsDto,
    pub parity: SerialParityDto,
    pub stop_bits: SerialStopBitsDto,
    pub flow_control: SerialFlowControlDto,
}

impl SerialConfig {
    pub fn from_input(input: SerialOpenInputDto) -> Self {
        Self {
            port_name: input.port_name,
            baud_rate: input.baud_rate,
            data_bits: input.data_bits,
            parity: input.parity,
            stop_bits: input.stop_bits,
            flow_control: input.flow_control,
        }
    }

    pub fn to_dto(&self) -> SerialConfigDto {
        SerialConfigDto {
            port_name: self.port_name.clone(),
            baud_rate: self.baud_rate,
            data_bits: self.data_bits,
            parity: self.parity,
            stop_bits: self.stop_bits,
            flow_control: self.flow_control,
        }
    }
}

/// 会话状态迁移的唯一入口：任何状态变化都必须经由此处
pub(crate) enum Transition<'a> {
    Opened(&'a SerialConfig),
    Closed { error: Option<SerialDebugErrorDto> },
}

struct SessionState {
    generation: u64,
    open: bool,
    config: Option<SerialConfig>,
    rx_bytes: u64,
    tx_bytes: u64,
    error: Option<SerialDebugErrorDto>,
}

impl SessionState {
    fn new() -> Self {
        Self {
            generation: 0,
            open: false,
            config: None,
            rx_bytes: 0,
            tx_bytes: 0,
            error: None,
        }
    }

    fn transition(&mut self, step: Transition<'_>) -> SerialSessionDto {
        match step {
            Transition::Opened(config) => {
                self.generation = self.generation.wrapping_add(1);
                self.open = true;
                self.config = Some(config.clone());
                self.rx_bytes = 0;
                self.tx_bytes = 0;
                self.error = None;
            }
            Transition::Closed { error } => {
                self.open = false;
                self.error = error;
            }
        }
        self.snapshot()
    }

    fn snapshot(&self) -> SerialSessionDto {
        SerialSessionDto {
            session_id: self.generation,
            open: self.open,
            port_name: self.config.as_ref().map(|config| config.port_name.clone()),
            config: self.config.as_ref().map(SerialConfig::to_dto),
            rx_bytes: self.rx_bytes,
            tx_bytes: self.tx_bytes,
            error: self.error.clone(),
        }
    }
}

/// 会话状态 + 端口句柄 + 读线程控制：三者在同一处，避免句柄与状态分家
pub struct SerialState {
    session: Mutex<SessionState>,
    handle: HandleSlot,
    stop: AtomicBool,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl Default for SerialState {
    fn default() -> Self {
        Self::new()
    }
}

impl SerialState {
    pub(crate) fn new() -> Self {
        Self {
            session: Mutex::new(SessionState::new()),
            handle: Arc::new(Mutex::new(None)),
            stop: AtomicBool::new(false),
            worker: Mutex::new(None),
        }
    }

    pub fn snapshot(&self) -> SerialSessionDto {
        self.lock_session().snapshot()
    }

    pub(crate) fn is_open(&self) -> bool {
        self.lock_session().open
    }

    pub(crate) fn generation(&self) -> u64 {
        self.lock_session().generation
    }

    pub(crate) fn apply(&self, step: Transition<'_>) -> SerialSessionDto {
        self.lock_session().transition(step)
    }

    /// 只在"仍开着"时迁移到关闭态，返回 `None` 表示已是关闭态（幂等，避免重复出站）
    pub(crate) fn close_if_open(
        &self,
        error: Option<SerialDebugErrorDto>,
    ) -> Option<SerialSessionDto> {
        let mut session = self.lock_session();
        if !session.open {
            return None;
        }
        Some(session.transition(Transition::Closed { error }))
    }

    pub(crate) fn add_rx(&self, bytes: u64) {
        let mut session = self.lock_session();
        session.rx_bytes = session.rx_bytes.saturating_add(bytes);
    }

    pub(crate) fn add_tx(&self, bytes: u64) {
        let mut session = self.lock_session();
        session.tx_bytes = session.tx_bytes.saturating_add(bytes);
    }

    pub(crate) fn handle_slot(&self) -> HandleSlot {
        Arc::clone(&self.handle)
    }

    pub(crate) fn install_handle(&self, handle: Box<dyn PortHandle>) {
        *self.lock_handle() = Some(handle);
    }

    pub(crate) fn take_handle(&self) -> Option<Box<dyn PortHandle>> {
        self.lock_handle().take()
    }

    pub(crate) fn stop_requested(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    pub(crate) fn request_stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }

    pub(crate) fn reset_stop(&self) {
        self.stop.store(false, Ordering::SeqCst);
    }

    pub(crate) fn set_worker(&self, worker: JoinHandle<()>) {
        *self.lock_worker() = Some(worker);
    }

    pub(crate) fn join_worker(&self) {
        let worker = self.lock_worker().take();
        if let Some(worker) = worker {
            let _ = worker.join();
        }
    }

    fn lock_session(&self) -> MutexGuard<'_, SessionState> {
        self.session
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn lock_handle(&self) -> MutexGuard<'_, Option<Box<dyn PortHandle>>> {
        self.handle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn lock_worker(&self) -> MutexGuard<'_, Option<JoinHandle<()>>> {
        self.worker
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
