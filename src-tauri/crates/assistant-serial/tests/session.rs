use std::collections::VecDeque;
use std::io;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use assistant_protocol::{
    SerialDataBitsDto, SerialDebugErrorCodeDto, SerialFlowControlDto, SerialOpenInputDto,
    SerialParityDto, SerialPortInfoDto, SerialPortKindDto, SerialRxBatchDto, SerialSessionDto,
    SerialSetSignalsInputDto, SerialStopBitsDto,
};
use assistant_serial::{
    EventSink, PortBackend, PortHandle, SerialConfig, SerialError, SerialEvent, SerialRuntime,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};

const EMPTY_READ_SLEEP_MS: u64 = 8;

#[derive(Default)]
struct FakePort {
    pending: Mutex<VecDeque<Vec<u8>>>,
    read_fails: AtomicBool,
    write_fails: AtomicBool,
    signals_fail: AtomicBool,
    written: Mutex<Vec<u8>>,
    flushes: AtomicUsize,
    dtr: Mutex<Vec<bool>>,
    rts: Mutex<Vec<bool>>,
    released: AtomicBool,
}

struct FakeHandle {
    port: Arc<FakePort>,
}

impl Drop for FakeHandle {
    fn drop(&mut self) {
        self.port.released.store(true, Ordering::SeqCst);
    }
}

impl PortHandle for FakeHandle {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.port.read_fails.load(Ordering::SeqCst) {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "设备已断开"));
        }
        let mut pending = self.port.pending.lock().unwrap();
        if let Some(chunk) = pending.pop_front() {
            let take = chunk.len().min(buf.len());
            buf[..take].copy_from_slice(&chunk[..take]);
            if take < chunk.len() {
                pending.push_front(chunk[take..].to_vec());
            }
            return Ok(take);
        }
        drop(pending);
        thread::sleep(Duration::from_millis(EMPTY_READ_SLEEP_MS));
        Err(io::Error::new(io::ErrorKind::TimedOut, "读超时"))
    }

    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        if self.port.write_fails.load(Ordering::SeqCst) {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "写入失败"));
        }
        self.port.written.lock().unwrap().extend_from_slice(bytes);
        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.port.flushes.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn set_dtr(&mut self, level: bool) -> Result<(), SerialError> {
        if self.port.signals_fail.load(Ordering::SeqCst) {
            return Err(SerialError::Unsupported("平台不支持 DTR".to_string()));
        }
        self.port.dtr.lock().unwrap().push(level);
        Ok(())
    }

    fn set_rts(&mut self, level: bool) -> Result<(), SerialError> {
        if self.port.signals_fail.load(Ordering::SeqCst) {
            return Err(SerialError::Unsupported("平台不支持 RTS".to_string()));
        }
        self.port.rts.lock().unwrap().push(level);
        Ok(())
    }
}

struct FakeBackend {
    ports: Vec<SerialPortInfoDto>,
    port: Arc<FakePort>,
    open_failure: Mutex<Option<SerialError>>,
    opened: Mutex<Vec<SerialConfig>>,
}

impl PortBackend for FakeBackend {
    fn list(&self) -> Result<Vec<SerialPortInfoDto>, SerialError> {
        Ok(self.ports.clone())
    }

    fn open(&self, config: &SerialConfig) -> Result<Box<dyn PortHandle>, SerialError> {
        self.opened.lock().unwrap().push(config.clone());
        if let Some(error) = self.open_failure.lock().unwrap().clone() {
            return Err(error);
        }
        Ok(Box::new(FakeHandle {
            port: Arc::clone(&self.port),
        }))
    }
}

#[derive(Default)]
struct Events {
    rx: Vec<SerialRxBatchDto>,
    status: Vec<SerialSessionDto>,
}

struct Harness {
    runtime: SerialRuntime,
    port: Arc<FakePort>,
    backend: Arc<FakeBackend>,
    events: Arc<Mutex<Events>>,
}

impl Harness {
    fn new() -> Self {
        let port = Arc::new(FakePort::default());
        let backend = Arc::new(FakeBackend {
            ports: vec![port_info("COM2"), port_info("COM10")],
            port: Arc::clone(&port),
            open_failure: Mutex::new(None),
            opened: Mutex::new(Vec::new()),
        });
        let runtime = SerialRuntime::new(Arc::clone(&backend) as Arc<dyn PortBackend>);
        let events = Arc::new(Mutex::new(Events::default()));
        let sink_events = Arc::clone(&events);
        let sink: EventSink = Arc::new(move |event: SerialEvent| {
            let mut guard = sink_events.lock().unwrap();
            match event {
                SerialEvent::Rx(batch) => guard.rx.push(batch),
                SerialEvent::Status(snapshot) => guard.status.push(snapshot),
            }
        });
        runtime.set_sink(sink);
        Self {
            runtime,
            port,
            backend,
            events,
        }
    }

    fn rx(&self) -> Vec<SerialRxBatchDto> {
        self.events.lock().unwrap().rx.clone()
    }

    fn status(&self) -> Vec<SerialSessionDto> {
        self.events.lock().unwrap().status.clone()
    }

    fn push(&self, bytes: &[u8]) {
        self.port.pending.lock().unwrap().push_back(bytes.to_vec());
    }

    fn wait_until(&self, mut predicate: impl FnMut(&Self) -> bool) {
        let deadline = Instant::now() + Duration::from_millis(800);
        while Instant::now() < deadline {
            if predicate(self) {
                return;
            }
            thread::sleep(Duration::from_millis(5));
        }
        panic!("等待条件超时");
    }

    fn wait_rx_count(&self, count: usize) {
        self.wait_until(|harness| harness.rx().len() >= count);
    }

    fn wait_status_count(&self, count: usize) {
        self.wait_until(|harness| harness.status().len() >= count);
    }
}

fn port_info(name: &str) -> SerialPortInfoDto {
    SerialPortInfoDto {
        port_name: name.to_string(),
        kind: SerialPortKindDto::Unknown,
        vid: None,
        pid: None,
        serial_number: None,
        manufacturer: None,
        product: None,
    }
}

fn open_input(name: &str, baud_rate: u32) -> SerialOpenInputDto {
    SerialOpenInputDto {
        port_name: name.to_string(),
        baud_rate,
        data_bits: SerialDataBitsDto::Eight,
        parity: SerialParityDto::None,
        stop_bits: SerialStopBitsDto::One,
        flow_control: SerialFlowControlDto::None,
    }
}

fn decode(batch: &SerialRxBatchDto) -> Vec<u8> {
    STANDARD.decode(&batch.data_base64).unwrap()
}

#[test]
fn open_emits_status_and_snapshot() {
    let harness = Harness::new();
    let snapshot = harness.runtime.open(open_input("COM2", 115_200)).unwrap();

    assert!(snapshot.open);
    assert_eq!(snapshot.session_id, 1);
    assert_eq!(snapshot.port_name.as_deref(), Some("COM2"));
    assert_eq!(snapshot.rx_bytes, 0);
    assert_eq!(snapshot.tx_bytes, 0);
    assert!(snapshot.error.is_none());
    assert_eq!(
        snapshot.config.map(|config| config.baud_rate),
        Some(115_200)
    );
    assert_eq!(harness.status().len(), 1);
    assert_eq!(harness.backend.opened.lock().unwrap().len(), 1);
}

#[test]
fn open_rejects_when_already_open() {
    let harness = Harness::new();
    harness.runtime.open(open_input("COM2", 9_600)).unwrap();
    let error = harness
        .runtime
        .open(open_input("COM10", 9_600))
        .unwrap_err();

    assert_eq!(error, SerialError::AlreadyOpen);
    assert_eq!(harness.status().len(), 1);
    assert_eq!(harness.backend.opened.lock().unwrap().len(), 1);
    assert_eq!(harness.runtime.snapshot().session_id, 1);
}

#[test]
fn open_validates_input_before_touching_port() {
    let harness = Harness::new();

    for (input, label) in [
        (open_input("", 115_200), "空端口名"),
        (open_input("COM2", 0), "零波特率"),
        (open_input("COM2", 4_000_001), "超上界波特率"),
    ] {
        let error = harness.runtime.open(input).unwrap_err();
        assert_eq!(
            error.code(),
            SerialDebugErrorCodeDto::InvalidInput,
            "{label}"
        );
    }

    assert!(harness.backend.opened.lock().unwrap().is_empty());
    assert!(harness.status().is_empty());
    assert!(!harness.runtime.snapshot().open);
}

#[test]
fn open_failure_keeps_session_closed() {
    let harness = Harness::new();
    *harness.backend.open_failure.lock().unwrap() =
        Some(SerialError::PortNotFound("COM2 不存在".to_string()));

    let error = harness
        .runtime
        .open(open_input("COM2", 115_200))
        .unwrap_err();
    assert_eq!(error.code(), SerialDebugErrorCodeDto::PortNotFound);

    let snapshot = harness.runtime.snapshot();
    assert!(!snapshot.open);
    assert_eq!(snapshot.session_id, 0);
    assert!(harness.status().is_empty());
}

#[test]
fn write_requires_open_and_rejects_empty_payload() {
    let harness = Harness::new();

    assert_eq!(
        harness.runtime.write("QQ==").unwrap_err(),
        SerialError::NotOpen
    );

    harness.runtime.open(open_input("COM2", 115_200)).unwrap();
    let error = harness.runtime.write("").unwrap_err();
    assert_eq!(error.code(), SerialDebugErrorCodeDto::InvalidInput);
    let error = harness.runtime.write("!!!").unwrap_err();
    assert_eq!(error.code(), SerialDebugErrorCodeDto::InvalidInput);
    assert!(harness.port.written.lock().unwrap().is_empty());
    assert!(harness.runtime.snapshot().open);
}

#[test]
fn write_counts_bytes_and_flushes() {
    let harness = Harness::new();
    harness.runtime.open(open_input("COM2", 115_200)).unwrap();

    let payload = STANDARD.encode(b"hello");
    let snapshot = harness.runtime.write(&payload).unwrap();

    assert_eq!(snapshot.tx_bytes, 5);
    assert_eq!(harness.port.written.lock().unwrap().as_slice(), b"hello");
    assert_eq!(harness.port.flushes.load(Ordering::SeqCst), 1);
    assert_eq!(harness.status().len(), 1);
}

#[test]
fn batch_flushes_at_byte_limit_then_interval() {
    let harness = Harness::new();
    harness.runtime.open(open_input("COM2", 115_200)).unwrap();

    for _ in 0..4 {
        harness.push(&[b'A'; 1024]);
    }
    harness.wait_rx_count(1);

    let batches = harness.rx();
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].session_id, 1);
    assert_eq!(batches[0].seq, 1);
    assert!(batches[0].received_at_ms > 0);
    assert_eq!(decode(&batches[0]).len(), 4096);

    harness.push(&[b'B'; 1024]);
    harness.wait_rx_count(2);
    let batches = harness.rx();
    assert_eq!(batches[1].seq, 2);
    assert_eq!(decode(&batches[1]).len(), 1024);
    assert!(harness.runtime.snapshot().rx_bytes >= 4096);
}

#[test]
fn idle_bytes_aggregate_into_single_batch() {
    let harness = Harness::new();
    harness.runtime.open(open_input("COM2", 115_200)).unwrap();

    for _ in 0..3 {
        harness.push(&[b'C'; 3]);
    }
    harness.wait_rx_count(1);
    thread::sleep(Duration::from_millis(40));

    let batches = harness.rx();
    assert_eq!(batches.len(), 1);
    assert_eq!(decode(&batches[0]), vec![b'C'; 9]);
    assert_eq!(harness.runtime.snapshot().rx_bytes, 9);
}

#[test]
fn read_failure_closes_session_and_keeps_counters() {
    let harness = Harness::new();
    harness.runtime.open(open_input("COM2", 115_200)).unwrap();

    harness.push(&[b'D'; 5]);
    harness.wait_rx_count(1);
    harness.port.read_fails.store(true, Ordering::SeqCst);
    harness.wait_status_count(2);

    let status = harness.status();
    let closed = status.last().unwrap();
    assert!(!closed.open);
    assert_eq!(closed.rx_bytes, 5);
    assert_eq!(
        closed.error.as_ref().map(|error| error.code),
        Some(SerialDebugErrorCodeDto::Io)
    );
    assert_eq!(harness.rx().len(), 1);
    assert_eq!(decode(&harness.rx()[0]), vec![b'D'; 5]);
    assert!(harness.port.released.load(Ordering::SeqCst));
    assert!(harness.runtime.write("QQ==").is_err());
}

#[test]
fn write_failure_closes_session() {
    let harness = Harness::new();
    harness.runtime.open(open_input("COM2", 115_200)).unwrap();
    harness.port.write_fails.store(true, Ordering::SeqCst);

    let error = harness.runtime.write(&STANDARD.encode(b"x")).unwrap_err();
    assert_eq!(error.code(), SerialDebugErrorCodeDto::Io);
    harness.wait_status_count(2);

    let snapshot = harness.runtime.snapshot();
    assert!(!snapshot.open);
    assert_eq!(snapshot.tx_bytes, 0);
    assert_eq!(
        snapshot.error.map(|error| error.code),
        Some(SerialDebugErrorCodeDto::Io)
    );
}

#[test]
fn signals_unsupported_keeps_session_open() {
    let harness = Harness::new();
    harness.runtime.open(open_input("COM2", 115_200)).unwrap();
    harness.port.signals_fail.store(true, Ordering::SeqCst);

    let error = harness
        .runtime
        .set_signals(SerialSetSignalsInputDto {
            dtr: Some(true),
            rts: None,
        })
        .unwrap_err();

    assert_eq!(error.code(), SerialDebugErrorCodeDto::Unsupported);
    assert!(harness.runtime.snapshot().open);
    assert_eq!(harness.status().len(), 1);
}

#[test]
fn signals_without_fields_is_noop() {
    let harness = Harness::new();
    harness.runtime.open(open_input("COM2", 115_200)).unwrap();

    let snapshot = harness
        .runtime
        .set_signals(SerialSetSignalsInputDto {
            dtr: None,
            rts: None,
        })
        .unwrap();

    assert!(snapshot.open);
    assert!(harness.port.dtr.lock().unwrap().is_empty());
    assert!(harness.port.rts.lock().unwrap().is_empty());
}

#[test]
fn signals_apply_only_given_fields() {
    let harness = Harness::new();
    harness.runtime.open(open_input("COM2", 115_200)).unwrap();

    harness
        .runtime
        .set_signals(SerialSetSignalsInputDto {
            dtr: Some(true),
            rts: Some(false),
        })
        .unwrap();

    assert_eq!(*harness.port.dtr.lock().unwrap(), vec![true]);
    assert_eq!(*harness.port.rts.lock().unwrap(), vec![false]);
}

#[test]
fn close_is_idempotent_and_releases_handle() {
    let harness = Harness::new();
    harness.runtime.open(open_input("COM2", 115_200)).unwrap();
    assert!(!harness.port.released.load(Ordering::SeqCst));

    let closed = harness.runtime.close();
    assert!(!closed.open);
    assert_eq!(closed.session_id, 1);
    assert!(harness.port.released.load(Ordering::SeqCst));
    assert_eq!(harness.status().len(), 2);

    let again = harness.runtime.close();
    assert!(!again.open);
    assert_eq!(again.session_id, 1);
    assert_eq!(harness.status().len(), 2);
}

#[test]
fn close_before_open_returns_closed_snapshot() {
    let harness = Harness::new();
    let snapshot = harness.runtime.close();

    assert!(!snapshot.open);
    assert_eq!(snapshot.session_id, 0);
    assert!(snapshot.port_name.is_none());
    assert!(snapshot.config.is_none());
    assert!(harness.status().is_empty());
}

#[test]
fn reopen_bumps_generation_and_resets_seq() {
    let harness = Harness::new();
    harness.runtime.open(open_input("COM2", 115_200)).unwrap();
    harness.push(&[b'E'; 2]);
    harness.wait_rx_count(1);

    harness.runtime.close();
    let reopened = harness.runtime.open(open_input("COM2", 115_200)).unwrap();
    assert_eq!(reopened.session_id, 2);
    assert_eq!(reopened.rx_bytes, 0);
    assert!(reopened.error.is_none());

    harness.push(&[b'F'; 2]);
    harness.wait_rx_count(2);

    let batches = harness.rx();
    assert_eq!(batches.len(), 2);
    assert_eq!(batches[0].session_id, 1);
    assert_eq!(batches[0].seq, 1);
    assert_eq!(batches[1].session_id, 2);
    assert_eq!(batches[1].seq, 1);
    assert_eq!(decode(&batches[1]), vec![b'F'; 2]);
}

#[test]
fn status_reflects_live_state() {
    let harness = Harness::new();
    harness.runtime.open(open_input("COM2", 115_200)).unwrap();

    let snapshot = harness.runtime.snapshot();
    assert!(snapshot.open);
    assert_eq!(snapshot.session_id, 1);
    assert_eq!(harness.runtime.list_ports().unwrap().len(), 2);
}

#[test]
fn close_quietly_does_not_emit_status() {
    let harness = Harness::new();
    harness.runtime.open(open_input("COM2", 115_200)).unwrap();

    let snapshot = harness.runtime.close_quietly();
    assert!(!snapshot.open);
    assert_eq!(harness.status().len(), 1);
}
