use std::cmp::Ordering;

use assistant_serial::backend::{device_path, natural_cmp};

#[test]
fn natural_order_puts_small_numbers_first() {
    assert_eq!(natural_cmp("COM2", "COM10"), Ordering::Less);
    assert_eq!(natural_cmp("COM10", "COM2"), Ordering::Greater);
    assert_eq!(natural_cmp("COM2", "COM2"), Ordering::Equal);
    assert_eq!(natural_cmp("/dev/ttyUSB0", "/dev/ttyUSB10"), Ordering::Less);
    assert_eq!(natural_cmp("/dev/ttyUSB9", "/dev/ttyUSB10"), Ordering::Less);
}

#[test]
fn natural_order_ignores_numeric_padding() {
    assert_eq!(natural_cmp("COM02", "COM2"), Ordering::Greater);
    assert_eq!(natural_cmp("COM2", "COM02"), Ordering::Less);
}

#[test]
fn natural_order_falls_back_to_text() {
    assert_eq!(natural_cmp("COM1", "COMX"), Ordering::Less);
    assert_eq!(natural_cmp("COMX", "COM1"), Ordering::Greater);
    assert_eq!(natural_cmp("COM", "COM1"), Ordering::Less);
}

#[cfg(windows)]
#[test]
fn windows_device_path_adds_extended_prefix() {
    assert_eq!(device_path("COM10"), r"\\.\COM10");
    assert_eq!(device_path(r"\\.\COM3"), r"\\.\COM3");
}
