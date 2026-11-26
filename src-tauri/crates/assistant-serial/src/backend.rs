use std::cmp::Ordering;
use std::io::{self, Read, Write};
use std::time::Duration;

use assistant_protocol::{
    SerialDataBitsDto, SerialFlowControlDto, SerialParityDto, SerialPortInfoDto, SerialPortKindDto,
    SerialStopBitsDto,
};
use serialport::{DataBits, ErrorKind, FlowControl, Parity, SerialPort, SerialPortType, StopBits};

use crate::error::SerialError;
use crate::state::SerialConfig;
use crate::READ_TIMEOUT_MS;

/// 端口句柄：领域层只认这组最小能力，平台实现由 `SerialPortBackend` 提供
pub trait PortHandle: Send {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>;
    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()>;
    fn flush(&mut self) -> io::Result<()>;
    fn set_dtr(&mut self, level: bool) -> Result<(), SerialError>;
    fn set_rts(&mut self, level: bool) -> Result<(), SerialError>;
}

/// 平台后端：枚举与打开，测试用假实现替换
pub trait PortBackend: Send + Sync {
    fn list(&self) -> Result<Vec<SerialPortInfoDto>, SerialError>;
    fn open(&self, config: &SerialConfig) -> Result<Box<dyn PortHandle>, SerialError>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SerialPortBackend;

impl PortBackend for SerialPortBackend {
    fn list(&self) -> Result<Vec<SerialPortInfoDto>, SerialError> {
        let mut ports: Vec<SerialPortInfoDto> = serialport::available_ports()
            .map_err(classify)?
            .into_iter()
            .map(port_info_dto)
            .collect();
        ports.sort_by(|left, right| natural_cmp(&left.port_name, &right.port_name));
        Ok(ports)
    }

    fn open(&self, config: &SerialConfig) -> Result<Box<dyn PortHandle>, SerialError> {
        let parity = map_parity(config.parity)?;
        let port = serialport::new(device_path(&config.port_name), config.baud_rate)
            .data_bits(map_data_bits(config.data_bits))
            .parity(parity)
            .stop_bits(map_stop_bits(config.stop_bits))
            .flow_control(map_flow_control(config.flow_control))
            .timeout(Duration::from_millis(READ_TIMEOUT_MS))
            .open()
            .map_err(classify)?;
        Ok(Box::new(SerialPortHandle { port }))
    }
}

struct SerialPortHandle {
    port: Box<dyn SerialPort>,
}

impl PortHandle for SerialPortHandle {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.port.read(buf)
    }

    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.port.write_all(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.port.flush()
    }

    fn set_dtr(&mut self, level: bool) -> Result<(), SerialError> {
        self.port
            .write_data_terminal_ready(level)
            .map_err(signal_error)
    }

    fn set_rts(&mut self, level: bool) -> Result<(), SerialError> {
        self.port.write_request_to_send(level).map_err(signal_error)
    }
}

fn classify(error: serialport::Error) -> SerialError {
    match error.kind() {
        ErrorKind::NoDevice => SerialError::PortNotFound(error.description),
        ErrorKind::InvalidInput => SerialError::InvalidInput(error.description),
        ErrorKind::Unknown => SerialError::Io(error.description),
        ErrorKind::Io(kind) => match kind {
            io::ErrorKind::NotFound => SerialError::PortNotFound(error.description),
            io::ErrorKind::PermissionDenied => SerialError::PermissionDenied(error.description),
            io::ErrorKind::WouldBlock | io::ErrorKind::AlreadyExists => {
                SerialError::Busy(error.description)
            }
            _ => SerialError::Io(error.description),
        },
    }
}

/// 信号线不支持时平台没有专门的错误类别，只能按"参数被拒/未知"归 `unsupported`
fn signal_error(error: serialport::Error) -> SerialError {
    match error.kind() {
        ErrorKind::InvalidInput | ErrorKind::Unknown => SerialError::Unsupported(error.description),
        _ => classify(error),
    }
}

fn port_info_dto(info: serialport::SerialPortInfo) -> SerialPortInfoDto {
    let (kind, vid, pid, serial_number, manufacturer, product) = match info.port_type {
        SerialPortType::UsbPort(usb) => (
            SerialPortKindDto::Usb,
            Some(usb.vid),
            Some(usb.pid),
            usb.serial_number,
            usb.manufacturer,
            usb.product,
        ),
        SerialPortType::BluetoothPort => {
            (SerialPortKindDto::Bluetooth, None, None, None, None, None)
        }
        SerialPortType::PciPort => (SerialPortKindDto::Pci, None, None, None, None, None),
        SerialPortType::Unknown => (SerialPortKindDto::Unknown, None, None, None, None, None),
    };
    SerialPortInfoDto {
        port_name: info.port_name,
        kind,
        vid,
        pid,
        serial_number,
        manufacturer,
        product,
    }
}

fn map_data_bits(data_bits: SerialDataBitsDto) -> DataBits {
    match data_bits {
        SerialDataBitsDto::Five => DataBits::Five,
        SerialDataBitsDto::Six => DataBits::Six,
        SerialDataBitsDto::Seven => DataBits::Seven,
        SerialDataBitsDto::Eight => DataBits::Eight,
    }
}

fn map_parity(parity: SerialParityDto) -> Result<Parity, SerialError> {
    match parity {
        SerialParityDto::None => Ok(Parity::None),
        SerialParityDto::Odd => Ok(Parity::Odd),
        SerialParityDto::Even => Ok(Parity::Even),
        SerialParityDto::Mark | SerialParityDto::Space => Err(SerialError::InvalidInput(
            "当前平台不支持 Mark/Space 校验位".to_string(),
        )),
    }
}

fn map_stop_bits(stop_bits: SerialStopBitsDto) -> StopBits {
    match stop_bits {
        SerialStopBitsDto::One => StopBits::One,
        SerialStopBitsDto::Two => StopBits::Two,
    }
}

fn map_flow_control(flow_control: SerialFlowControlDto) -> FlowControl {
    match flow_control {
        SerialFlowControlDto::None => FlowControl::None,
        SerialFlowControlDto::Software => FlowControl::Software,
        SerialFlowControlDto::Hardware => FlowControl::Hardware,
    }
}

/// Windows 的 `COM10` 及以上必须走 `\\.\COM10`；`portName` 对外仍是裸名
pub fn device_path(port_name: &str) -> String {
    if cfg!(windows) {
        if port_name.starts_with(r"\\.\") {
            port_name.to_string()
        } else {
            format!(r"\\.\{port_name}")
        }
    } else if cfg!(target_os = "macos") && port_name.starts_with("/dev/tty.") {
        port_name.replacen("/dev/tty.", "/dev/cu.", 1)
    } else {
        port_name.to_string()
    }
}

/// 自然序：`COM2` 排在 `COM10` 前
pub fn natural_cmp(left: &str, right: &str) -> Ordering {
    let left_chunks = split_chunks(left);
    let right_chunks = split_chunks(right);
    for (index, (left_digits, left_text)) in left_chunks.iter().enumerate() {
        let Some((right_digits, right_text)) = right_chunks.get(index) else {
            return Ordering::Greater;
        };
        let order = match (left_digits, right_digits) {
            (true, true) => {
                let left_number = left_text.trim_start_matches('0');
                let right_number = right_text.trim_start_matches('0');
                left_number
                    .len()
                    .cmp(&right_number.len())
                    .then_with(|| left_number.cmp(right_number))
                    .then_with(|| left_text.len().cmp(&right_text.len()))
            }
            (false, false) => left_text.cmp(right_text),
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
        };
        if order != Ordering::Equal {
            return order;
        }
    }
    if right_chunks.len() > left_chunks.len() {
        Ordering::Less
    } else {
        Ordering::Equal
    }
}

fn split_chunks(text: &str) -> Vec<(bool, &str)> {
    let bytes = text.as_bytes();
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < bytes.len() {
        let digits = bytes[start].is_ascii_digit();
        let mut end = start + 1;
        while end < bytes.len() && bytes[end].is_ascii_digit() == digits {
            end += 1;
        }
        chunks.push((digits, &text[start..end]));
        start = end;
    }
    chunks
}
