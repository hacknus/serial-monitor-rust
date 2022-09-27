use std::fs::read;
use std::num::ParseIntError;
use std::ptr::write;
use std::str::Utf8Error;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use serialport::SerialPort;
use crate::{DataContainer, GuiSettingsContainer, Packet, Print, print_to_console};
use crate::data::SerialDirection;

pub fn serial_thread(gui_settings: GuiSettingsContainer,
                     device_lock: Arc<RwLock<String>>,
                     devices_lock: Arc<RwLock<Vec<String>>>,
                     baud_lock: Arc<RwLock<u32>>,
                     raw_data_lock: Arc<RwLock<Packet>>,
                     print_lock: Arc<RwLock<Vec<Print>>>,
                     connected_lock: Arc<RwLock<bool>>) {
    let mut device = "".to_string();
    let mut devices : Vec<String> = vec![];
    let mut baud_rate = 116_200;
    let mut connected = false;
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
            println!("device: {}", p.port_name);
            devices.push(p.port_name.clone());
            if p.port_name == device {
                connected = true;
                println!("connecting to serial port");
                break;
            }
        }
        if let Ok(mut write_guard) = devices_lock.write() {
            *write_guard = devices.clone();
        }
        println!(" ");
        std::thread::sleep(Duration::from_millis(100));
    }
    println!("device: {}, baudrate: {}", device, baud_rate);
    let mut port = serialport::new(&device, baud_rate)
        .timeout(Duration::from_millis(100));
    println!("created serial port");
    let mut port = port.open().unwrap();
    println!("opened serial port");
    loop {
        devices = vec![];
        for p in serialport::available_ports().unwrap().iter() {
            println!("device: {}", p.port_name);
            devices.push(p.port_name.clone());
            if p.port_name == device {
                connected = true;
                println!("connecting to serial port");
                break;
            }
        }
        if let Ok(mut write_guard) = devices_lock.write() {
            *write_guard = devices.clone();
        }

        let mut serial_buf: Vec<u8> = vec![0; 128];
        match port.read(serial_buf.as_mut_slice()) {
            Ok(_) => {
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
            }
            Err(_) => {}
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}