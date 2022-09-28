use std::fs::read;
use std::num::ParseIntError;
use std::ptr::write;
use std::str::Utf8Error;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use serialport::SerialPort;
use crate::{DataContainer, GuiSettingsContainer, Packet, Print, print_to_console};
use crate::data::SerialDirection;


fn serial_write(port: &mut Box<dyn SerialPort>, cmd: &[u8]) -> bool {
    match port.write(cmd) {
        Ok(_) => {
            let mut response = vec![0; 128];
            serial_read(port, &mut response);

            match std::str::from_utf8(&response) {
                Ok(v) => {
                    if v.contains("OK") {
                        true
                    } else {
                        false
                    }
                }
                Err(_) => { false }
            }
        }
        Err(_) => { false }
    }
}

fn serial_read(port: &mut Box<dyn SerialPort>, serial_buf: &mut Vec<u8>) -> bool {
    match port.read(serial_buf.as_mut_slice()) {
        Ok(_) => { true }
        Err(_) => { false }
    }
}

pub fn serial_thread(gui_settings: GuiSettingsContainer,
                     device_lock: Arc<RwLock<String>>,
                     devices_lock: Arc<RwLock<Vec<String>>>,
                     baud_lock: Arc<RwLock<u32>>,
                     raw_data_lock: Arc<RwLock<Packet>>,
                     print_lock: Arc<RwLock<Vec<Print>>>,
                     connected_lock: Arc<RwLock<bool>>) {
    let mut device = "".to_string();
    let mut devices: Vec<String> = vec![];
    let mut baud_rate = 116_200;
    let mut connected = false;
    if let Ok(mut write_guard) = connected_lock.write() {
        *write_guard = connected.clone();
    }
    let mut port: Box<dyn SerialPort>;
    while !connected {
        if let Ok(read_guard) = baud_lock.read() {
            baud_rate = read_guard.clone()
        }
        if let Ok(read_guard) = device_lock.read() {
            device = read_guard.clone();
        }
        devices = vec![];
        for p in serialport::available_ports().unwrap().iter() {
            devices.push(p.port_name.clone());
            if p.port_name == device {
                connected = true;
                break;
            }
        }
        if let Ok(mut write_guard) = devices_lock.write() {
            *write_guard = devices.clone();
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let mut port = serialport::new(&device, baud_rate)
        .timeout(Duration::from_millis(100));
    let mut port = port.open().unwrap();

    if let Ok(mut write_guard) = connected_lock.write() {
        *write_guard = connected.clone();
    }

    loop {

        // check for reconnection

        devices = vec![];
        for p in serialport::available_ports().unwrap().iter() {
            println!("device: {}", p.port_name);
            devices.push(p.port_name.clone());
        }
        if let Ok(mut write_guard) = devices_lock.write() {
            *write_guard = devices.clone();
        }

        // perform writes
        let output = "[CMD] set hk rate=1.0\r\n".as_bytes();
        serial_write(&mut port, &output);
        if let Ok(mut write_guard) = raw_data_lock.write() {
            match std::str::from_utf8(&output) {
                Ok(v) => {
                    let packet = Packet {
                        time: Instant::now(),
                        direction: SerialDirection::SEND,
                        payload: v.to_string(),
                    };
                    *write_guard = packet;
                }
                Err(_) => {}
            }
        }

        // perform reads
        let mut serial_buf: Vec<u8> = vec![0; 128];
        serial_read(&mut port, &mut serial_buf);
        if let Ok(mut write_guard) = raw_data_lock.write() {
            match std::str::from_utf8(&serial_buf) {
                Ok(v) => {
                    let packet = Packet {
                        time: Instant::now(),
                        direction: SerialDirection::RECEIVE,
                        payload: v.to_string(),
                    };
                    *write_guard = packet;
                }
                Err(_) => {}
            }
        }

        std::thread::sleep(Duration::from_millis(10));
    }
}