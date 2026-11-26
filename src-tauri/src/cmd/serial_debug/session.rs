use std::sync::Arc;

use tauri::{command, State};

use assistant_protocol::{
    SerialDebugErrorDto, SerialOpenInputDto, SerialSessionDto, SerialSetSignalsInputDto,
};
use assistant_serial::SerialRuntime;

#[command]
pub fn serial_debug_open(
    rt: State<'_, Arc<SerialRuntime>>,
    input: SerialOpenInputDto,
) -> Result<SerialSessionDto, SerialDebugErrorDto> {
    rt.open(input).map_err(SerialDebugErrorDto::from)
}

#[command]
pub fn serial_debug_close(
    rt: State<'_, Arc<SerialRuntime>>,
) -> Result<SerialSessionDto, SerialDebugErrorDto> {
    Ok(rt.close())
}

#[command]
pub fn serial_debug_status(
    rt: State<'_, Arc<SerialRuntime>>,
) -> Result<SerialSessionDto, SerialDebugErrorDto> {
    Ok(rt.snapshot())
}

#[command]
pub fn serial_debug_write(
    rt: State<'_, Arc<SerialRuntime>>,
    data_base64: String,
) -> Result<SerialSessionDto, SerialDebugErrorDto> {
    rt.write(&data_base64).map_err(SerialDebugErrorDto::from)
}

#[command]
pub fn serial_debug_set_signals(
    rt: State<'_, Arc<SerialRuntime>>,
    input: SerialSetSignalsInputDto,
) -> Result<SerialSessionDto, SerialDebugErrorDto> {
    rt.set_signals(input).map_err(SerialDebugErrorDto::from)
}
