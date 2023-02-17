use std::io::{BufRead, BufReader};
use std::sync::{Arc, RwLock};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};
use serialport::SerialPort;
use crate::{GuiSettingsContainer, Packet, Print, print_to_console};
use crate::data::SerialDirection;


fn serial_write(port: &mut BufReader<Box<dyn SerialPort>>, cmd: &[u8]) -> bool {
    let write_port = port.get_mut();
    match write_port.write(cmd) {
        Ok(_) => {
            let mut response = "".to_string();
            serial_read(port, &mut response);
            println!("sent a command!");
            if response.contains("OK") {
                true
            } else {
                println!("cmd not acknowledged!!!");
                false
            }
        }
        Err(_) => { false }
    }
}

fn serial_read(port: &mut BufReader<Box<dyn SerialPort>>, serial_buf: &mut String) -> bool {
    match port.read_line(serial_buf) {
        Ok(_) => { true }
        Err(_) => {
            // this probably means that either there is no data,
            // or it could not be decoded to a String (binary stuff...)
            false
        }
    }
}

pub fn serial_thread(gui_settings: GuiSettingsContainer,
                     send_rx: Receiver<String>,
                     device_lock: Arc<RwLock<String>>,
                     devices_lock: Arc<RwLock<Vec<String>>>,
                     baud_lock: Arc<RwLock<u32>>,
                     raw_data_lock: Arc<RwLock<Vec<Packet>>>,
                     print_lock: Arc<RwLock<Vec<Print>>>,
                     connected_lock: Arc<RwLock<bool>>) {
    let mut device = "".to_string();
    let mut devices: Vec<String>;
    let mut baud_rate = 115_200;
    let mut connected;
    loop {

        let _not_awake = keepawake::Builder::new()
            .display(false)
            .reason("Serial Connection")
            .app_name("Serial Monitor")
            //.app_reverse_domain("io.github.myprog")
            .create()
            .unwrap();

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
                    // break;
                }
            }
            if let Ok(mut write_guard) = devices_lock.write() {
                *write_guard = devices.clone();
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let port_builder = serialport::new(&device, baud_rate)
            .timeout(Duration::from_millis(100));
        let port;
        match port_builder.open() {
            Ok(p) => {port=p}
            Err(err) => {
                if let Ok(mut write_guard) = device_lock.write() {
                    *write_guard = "".to_string();
                }
                device = "".to_string();
                print_to_console(&print_lock, Print::ERROR(format!("Error connecting: {}", err.to_string())));
                continue;
            }
        }
        let mut port = BufReader::new(port);

        if let Ok(mut write_guard) = connected_lock.write() {
            *write_guard = connected.clone();
        }

        let t_zero = Instant::now();

        print_to_console(&print_lock, Print::OK(format!("connected to serial port: {} @ baud = {}", device, baud_rate)));

        let mut reconnect = false;

        let _awake = keepawake::Builder::new()
            .display(true)
            .reason("Serial Connection")
            .app_name("Serial Monitor")
            //.app_reverse_domain("io.github.myprog")
            .create()
            .unwrap();

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
                print_to_console(&print_lock, Print::ERROR(format!("disconnected from serial port: {}", device)));
                if let Ok(mut write_guard) = device_lock.write() {
                    *write_guard = "".to_string();
                }
                break 'connected_loop;
            }

            // perform writes
            match send_rx.recv_timeout(Duration::from_millis(1)) {
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
                                write_guard.push(packet);
                            }
                            Err(_) => {
                                // println!("output encode fail");
                            }
                        }
                    }
                }
                Err(..) => {}
            }

            // perform reads
            let mut serial_buf = "".to_string();
            if serial_read(&mut port, &mut serial_buf) {
                if let Ok(mut write_guard) = raw_data_lock.write() {
                    // println!("received: {:?}", serial_buf);
                    let payloads: Vec<&str>;
                    if serial_buf.contains("\r\n") {
                        payloads = serial_buf.split("\r\n").collect::<Vec<&str>>();
                    } else {
                        payloads = serial_buf.split("\0\0").collect::<Vec<&str>>();
                    }
                    // println!("received split2: {:?}", payloads);
                    for payload in payloads.iter() {
                        let payload_string = payload.to_string();
                        if !payload_string.contains("\0\0") && payload_string != "".to_string() {
                            let packet = Packet {
                                time: Instant::now().duration_since(t_zero).as_millis(),
                                direction: SerialDirection::RECEIVE,
                                payload: payload_string,
                            };
                            write_guard.push(packet);
                        }
                    }
                }
            }

            //std::thread::sleep(Duration::from_millis(10));
        }
        std::mem::drop(port);
    }
}