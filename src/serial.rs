use std::fs::read;
use std::num::ParseIntError;
use std::ptr::write;
use std::str::Utf8Error;
use std::sync::{Arc, RwLock};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};
use serialport::SerialPort;
use crate::{DataContainer, GuiSettingsContainer, Packet, Print, print_to_console};
use crate::data::SerialDirection;


fn serial_write(port: &mut Box<dyn SerialPort>, cmd: &[u8]) -> bool {
    match port.write(cmd) {
        Ok(_) => {
            let mut response = vec![0; 128];
            serial_read(port, &mut response);
            println!("sent a command!");
            match std::str::from_utf8(&response) {
                Ok(v) => {
                    if v.contains("OK") {
                        true
                    } else {
                        println!("cmd not acknowledged!!!");
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
                     send_rx: Receiver<String>,
                     device_lock: Arc<RwLock<String>>,
                     devices_lock: Arc<RwLock<Vec<String>>>,
                     baud_lock: Arc<RwLock<u32>>,
                     raw_data_lock: Arc<RwLock<Packet>>,
                     print_lock: Arc<RwLock<Vec<Print>>>,
                     connected_lock: Arc<RwLock<bool>>) {
    let mut device = "".to_string();
    let mut devices: Vec<String> = vec![];
    let mut baud_rate = 115_200;
    let mut connected;
    loop {
        connected = false;
        if let Ok(mut write_guard) = connected_lock.write() {
            *write_guard = connected.clone();
        }

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
        let mut port_builder = serialport::new(&device, baud_rate)
            .timeout(Duration::from_millis(100));
        let mut port = port_builder.open().unwrap();

        if let Ok(mut write_guard) = connected_lock.write() {
            *write_guard = connected.clone();
        }

        let t_zero = Instant::now();

        println!("opened serial connection");
        let mut reconnect = false;

        'connected_loop: loop {

            // check for reconnection

            devices = vec![];
            for p in serialport::available_ports().unwrap().iter() {
                devices.push(p.port_name.clone());
            }
            if let Ok(mut write_guard) = devices_lock.write() {
                *write_guard = devices.clone();
            }

            if let Ok(read_guard) = baud_lock.read() {
                if baud_rate != *read_guard {
                    baud_rate = read_guard.clone();
                    reconnect = true;
                }
            }
            if let Ok(read_guard) = device_lock.read() {
                if device != *read_guard {
                    device = read_guard.clone();
                    reconnect = true;
                }
            }

            let mut dev_is_con = false;
            for dev in devices.iter() {
                if device == *dev {
                    dev_is_con = true;
                }
            }


            if reconnect || !dev_is_con {
                break 'connected_loop;
            }

            // perform writes
            match send_rx.recv_timeout(Duration::from_millis(10)) {
                Ok(cmd) => {
                    let output = cmd.as_bytes();
                    serial_write(&mut port, &output);
                    if let Ok(mut write_guard) = raw_data_lock.write() {
                        match std::str::from_utf8(&output) {
                            Ok(v) => {
                                let packet = Packet {
                                    time: Instant::now().duration_since(t_zero).as_millis(),
                                    direction: SerialDirection::SEND,
                                    payload: v.to_string(),
                                };
                                *write_guard = packet;
                            }
                            Err(_) => {
                                println!("output encode fail");
                            }
                        }
                    }
                }
                Err(..) => {}
            }


            // perform reads
            let mut serial_buf: Vec<u8> = vec![0; 1024];
            if serial_read(&mut port, &mut serial_buf) {
                if let Ok(mut write_guard) = raw_data_lock.write() {
                    match std::str::from_utf8(&serial_buf) {
                        Ok(v) => {
                            let p = v.to_string();
                            println!("received: {:?}", p);
                            let payloads: Vec<&str>;
                            if p.contains("\r\n") {
                                payloads = p.split("\r\n").collect::<Vec<&str>>();
                            } else {
                                payloads = p.split("\0\0").collect::<Vec<&str>>();
                            }
                            println!("received split2: {:?}", payloads);
                            for payload in payloads.iter() {
                                let payload_string = payload.to_string();
                                if !payload_string.contains("\0\0") && payload_string != "".to_string() {
                                    let packet = Packet {
                                        time: Instant::now().duration_since(t_zero).as_millis(),
                                        direction: SerialDirection::RECEIVE,
                                        payload: payload_string,
                                    };
                                    *write_guard = packet;
                                }
                            }
                        }
                        Err(_) => {
                            println!("recv encode fail");
                        }
                    }
                }
            }

            std::thread::sleep(Duration::from_millis(100));
        }
        std::mem::drop(port);
    }
}