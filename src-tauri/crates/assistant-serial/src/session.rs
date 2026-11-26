use std::sync::{Arc, Mutex, MutexGuard, RwLock};

use assistant_protocol::{
    SerialOpenInputDto, SerialPortInfoDto, SerialSessionDto, SerialSetSignalsInputDto,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};

use crate::backend::{PortBackend, PortHandle, SerialPortBackend};
use crate::error::SerialError;
use crate::event::{noop_sink, EventSink, SerialEvent};
use crate::reader;
use crate::state::{SerialConfig, SerialState, Transition};
use crate::BAUD_RATE_MAX;

/// 串口调试领域门面：状态、句柄、读线程、事件出口都从这里编排
pub struct SerialRuntime {
    backend: Arc<dyn PortBackend>,
    state: Arc<SerialState>,
    sink: RwLock<EventSink>,
    ops: Mutex<()>,
}

impl SerialRuntime {
    pub fn new(backend: Arc<dyn PortBackend>) -> Self {
        Self {
            backend,
            state: Arc::new(SerialState::new()),
            sink: RwLock::new(noop_sink()),
            ops: Mutex::new(()),
        }
    }

    pub fn with_default_backend() -> Self {
        Self::new(Arc::new(SerialPortBackend))
    }

    pub fn set_sink(&self, sink: EventSink) {
        *self
            .sink
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = sink;
    }

    pub fn snapshot(&self) -> SerialSessionDto {
        self.state.snapshot()
    }

    pub fn list_ports(&self) -> Result<Vec<SerialPortInfoDto>, SerialError> {
        self.backend.list()
    }

    pub fn open(&self, input: SerialOpenInputDto) -> Result<SerialSessionDto, SerialError> {
        let _ops = self.lock_ops();
        if self.state.is_open() {
            return Err(SerialError::AlreadyOpen);
        }
        let config = validate(input)?;
        self.state.request_stop();
        self.state.join_worker();
        self.state.take_handle();
        self.state.reset_stop();
        let handle = self.backend.open(&config)?;
        self.state.install_handle(handle);
        let snapshot = self.state.apply(Transition::Opened(&config));
        let worker = reader::spawn(Arc::clone(&self.state), snapshot.session_id, self.sink());
        self.state.set_worker(worker);
        log::info!(
            "串口已打开：{} @ {}（session {}）",
            config.port_name,
            config.baud_rate,
            snapshot.session_id
        );
        self.emit(SerialEvent::Status(snapshot.clone()));
        Ok(snapshot)
    }

    /// 幂等关闭：未开时返回 `open=false` 快照，不报错
    pub fn close(&self) -> SerialSessionDto {
        self.shutdown(true)
    }

    /// 退出流程用：关端口但不发事件（窗口已销毁，广播无意义）
    pub fn close_quietly(&self) -> SerialSessionDto {
        self.shutdown(false)
    }

    pub fn write(&self, data_base64: &str) -> Result<SerialSessionDto, SerialError> {
        let _ops = self.lock_ops();
        if !self.state.is_open() {
            return Err(SerialError::NotOpen);
        }
        let bytes = decode_payload(data_base64)?;
        let outcome = self.with_handle::<(), SerialError>(|handle| {
            handle.write_all(&bytes)?;
            handle.flush()?;
            Ok(())
        });
        match outcome {
            Ok(()) => {
                self.state.add_tx(bytes.len() as u64);
                Ok(self.state.snapshot())
            }
            Err(error) => {
                self.fail_session(error.clone());
                Err(error)
            }
        }
    }

    pub fn set_signals(
        &self,
        input: SerialSetSignalsInputDto,
    ) -> Result<SerialSessionDto, SerialError> {
        let _ops = self.lock_ops();
        if !self.state.is_open() {
            return Err(SerialError::NotOpen);
        }
        if let Some(level) = input.dtr {
            self.with_handle(|handle| handle.set_dtr(level))?;
        }
        if let Some(level) = input.rts {
            self.with_handle(|handle| handle.set_rts(level))?;
        }
        Ok(self.state.snapshot())
    }

    fn shutdown(&self, notify: bool) -> SerialSessionDto {
        let _ops = self.lock_ops();
        self.state.request_stop();
        self.state.join_worker();
        self.state.take_handle();
        match self.state.close_if_open(None) {
            Some(snapshot) => {
                log::info!("串口已关闭（session {}）", snapshot.session_id);
                if notify {
                    self.emit(SerialEvent::Status(snapshot.clone()));
                }
                snapshot
            }
            None => self.state.snapshot(),
        }
    }

    fn fail_session(&self, error: SerialError) {
        self.state.request_stop();
        self.state.take_handle();
        log::warn!("串口写入失败，会话转关闭：{error}");
        if let Some(snapshot) = self.state.close_if_open(Some(error.to_dto())) {
            self.emit(SerialEvent::Status(snapshot));
        }
    }

    fn with_handle<T, E>(
        &self,
        action: impl FnOnce(&mut dyn PortHandle) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<SerialError>,
    {
        let slot = self.state.handle_slot();
        let mut guard = slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(handle) = guard.as_mut() else {
            return Err(E::from(SerialError::NotOpen));
        };
        action(handle.as_mut())
    }

    fn emit(&self, event: SerialEvent) {
        let sink = self.sink();
        sink(event);
    }

    fn sink(&self) -> EventSink {
        self.sink
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn lock_ops(&self) -> MutexGuard<'_, ()> {
        self.ops
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn validate(input: SerialOpenInputDto) -> Result<SerialConfig, SerialError> {
    if input.port_name.trim().is_empty() {
        return Err(SerialError::InvalidInput("端口名不能为空".to_string()));
    }
    if input.baud_rate == 0 || input.baud_rate > BAUD_RATE_MAX {
        return Err(SerialError::InvalidInput(format!(
            "波特率必须在 1..={BAUD_RATE_MAX} 之间"
        )));
    }
    Ok(SerialConfig::from_input(input))
}

fn decode_payload(data_base64: &str) -> Result<Vec<u8>, SerialError> {
    if data_base64.is_empty() {
        return Err(SerialError::InvalidInput("写入数据为空".to_string()));
    }
    STANDARD
        .decode(data_base64)
        .map_err(|_| SerialError::InvalidInput("写入数据不是合法 base64".to_string()))
}
