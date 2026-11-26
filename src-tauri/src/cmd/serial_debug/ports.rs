use std::sync::Arc;

use tauri::{command, State};

use assistant_protocol::SerialPortInfoDto;
use assistant_serial::SerialRuntime;

use assistant_protocol::SerialDebugErrorDto;

#[command]
pub fn serial_debug_list_ports(
    rt: State<'_, Arc<SerialRuntime>>,
) -> Result<Vec<SerialPortInfoDto>, SerialDebugErrorDto> {
    rt.list_ports().map_err(SerialDebugErrorDto::from)
}
