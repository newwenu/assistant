use std::io;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use assistant_protocol::SerialRxBatchDto;
use base64::{engine::general_purpose::STANDARD, Engine as _};

use crate::error::SerialError;
use crate::event::{EventSink, SerialEvent};
use crate::state::SerialState;
use crate::{BATCH_BYTES, BATCH_INTERVAL_MS};

const READ_CHUNK_BYTES: usize = 1024;

pub(crate) fn spawn(state: Arc<SerialState>, generation: u64, sink: EventSink) -> JoinHandle<()> {
    thread::spawn(move || run(&state, generation, &sink))
}

fn run(state: &Arc<SerialState>, generation: u64, sink: &EventSink) {
    let mut batch = Batch::new();
    let mut chunk = [0u8; READ_CHUNK_BYTES];

    loop {
        if state.stop_requested() || state.generation() != generation {
            break;
        }
        let outcome = catch_unwind(AssertUnwindSafe(|| read_once(state, &mut chunk)));
        match outcome {
            Ok(Ok(count)) => {
                if count > 0 {
                    batch.push(&chunk[..count]);
                    state.add_rx(count as u64);
                }
                if batch.due() {
                    batch.flush(generation, sink);
                }
            }
            Ok(Err(error)) if is_timeout(&error) => {
                if batch.due() {
                    batch.flush(generation, sink);
                }
            }
            Ok(Err(error)) => {
                batch.flush(generation, sink);
                fail(state, generation, sink, SerialError::Io(error.to_string()));
                return;
            }
            Err(_) => {
                batch.flush(generation, sink);
                fail(
                    state,
                    generation,
                    sink,
                    SerialError::Io("读线程异常终止".to_string()),
                );
                return;
            }
        }
    }
    batch.flush(generation, sink);
}

fn read_once(state: &SerialState, chunk: &mut [u8]) -> io::Result<usize> {
    let slot = state.handle_slot();
    let mut guard = slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    match guard.as_mut() {
        Some(handle) => handle.read(chunk),
        None => Err(io::Error::new(io::ErrorKind::NotConnected, "端口已释放")),
    }
}

fn is_timeout(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
    )
}

fn fail(state: &Arc<SerialState>, generation: u64, sink: &EventSink, error: SerialError) {
    state.request_stop();
    state.take_handle();
    log::warn!("串口会话异常终止（session {generation}）：{error}");
    if let Some(snapshot) = state.close_if_open(Some(error.to_dto())) {
        sink(SerialEvent::Status(snapshot));
    }
}

struct Batch {
    bytes: Vec<u8>,
    started: Option<Instant>,
    received_at_ms: u64,
    seq: u64,
}

impl Batch {
    fn new() -> Self {
        Self {
            bytes: Vec::with_capacity(BATCH_BYTES),
            started: None,
            received_at_ms: 0,
            seq: 0,
        }
    }

    fn push(&mut self, chunk: &[u8]) {
        if self.started.is_none() {
            self.started = Some(Instant::now());
            self.received_at_ms = epoch_ms();
        }
        self.bytes.extend_from_slice(chunk);
    }

    fn due(&self) -> bool {
        let Some(started) = self.started else {
            return false;
        };
        self.bytes.len() >= BATCH_BYTES || started.elapsed().as_millis() as u64 >= BATCH_INTERVAL_MS
    }

    fn flush(&mut self, generation: u64, sink: &EventSink) {
        if self.bytes.is_empty() {
            self.started = None;
            return;
        }
        self.seq += 1;
        sink(SerialEvent::Rx(SerialRxBatchDto {
            session_id: generation,
            seq: self.seq,
            received_at_ms: self.received_at_ms,
            data_base64: STANDARD.encode(&self.bytes),
        }));
        self.bytes.clear();
        self.started = None;
    }
}

fn epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0)
}
