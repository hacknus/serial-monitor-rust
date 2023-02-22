use crate::data::SerialDirection;
use crate::{print_to_console, GuiSettingsContainer, Packet, Print};
use serialport::SerialPort;
use std::io::{BufRead, BufReader};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

fn serial_write(
    port: &mut BufReader<Box<dyn SerialPort>>,
    cmd: &[u8],
) -> Result<usize, std::io::Error> {
    let write_port = port.get_mut();
    write_port.write(cmd)
}

fn serial_read(
    port: &mut BufReader<Box<dyn SerialPort>>,
    serial_buf: &mut String,
) -> Result<usize, std::io::Error> {
    port.read_line(serial_buf)
}

pub fn serial_thread(
    gui_settings: GuiSettingsContainer,
    send_rx: Receiver<String>,
    device_lock: Arc<RwLock<String>>,
    devices_lock: Arc<RwLock<Vec<String>>>,
    baud_lock: Arc<RwLock<u32>>,
    raw_data_lock: Arc<RwLock<Vec<Packet>>>,
    print_lock: Arc<RwLock<Vec<Print>>>,
    connected_lock: Arc<RwLock<bool>>,
) {
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
            *write_guard = connected;
        }

        while !connected {
            if let Ok(read_guard) = baud_lock.read() {
                baud_rate = *read_guard
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
        let port_builder = serialport::new(&device, baud_rate).timeout(Duration::from_millis(100));
        let mut port = match port_builder.open() {
            Ok(p) => BufReader::new(p),
            Err(err) => {
                if let Ok(mut write_guard) = device_lock.write() {
                    *write_guard = "".to_string();
                }
                device = "".to_string();
                print_to_console(
                    &print_lock,
                    Print::Error(format!("Error connecting: {}", err)),
                );
                continue;
            }
        };

        if let Ok(mut write_guard) = connected_lock.write() {
            *write_guard = connected;
        }

        let t_zero = Instant::now();

        print_to_console(
            &print_lock,
            Print::OK(format!(
                "connected to serial port: {} @ baud = {}",
                device, baud_rate
            )),
        );

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
                    baud_rate = *read_guard;
                    reconnect = true;
                }
            }
            if let Ok(read_guard) = device_lock.read() {
                if device != *read_guard {
                    device = read_guard.clone();
                    reconnect = true;
                }
            }

            let dev_is_con = devices.contains(&device);

            if reconnect || !dev_is_con {
                print_to_console(
                    &print_lock,
                    Print::Error(format!("disconnected from serial port: {}", device)),
                );
                if let Ok(mut write_guard) = device_lock.write() {
                    *write_guard = "".to_string();
                }
                break 'connected_loop;
            }

            // perform writes
            if let Ok(cmd) = send_rx.recv_timeout(Duration::from_millis(1)) {
                if serial_write(&mut port, cmd.as_bytes()).is_ok() {
                    if let Ok(mut write_guard) = raw_data_lock.write() {
                        let packet = Packet {
                            time: Instant::now().duration_since(t_zero).as_millis(),
                            direction: SerialDirection::Send,
                            payload: cmd,
                        };
                        write_guard.push(packet);
                    }
                }
            }

            // perform reads
            let mut serial_buf = "".to_string();
            if serial_read(&mut port, &mut serial_buf).is_ok() {
                if let Ok(mut write_guard) = raw_data_lock.write() {
                    // println!("received: {:?}", serial_buf);
                    let delimiter = if serial_buf.contains("\r\n") {
                        "\r\n"
                    } else {
                        "\0\0"
                    };

                    serial_buf
                        .split(delimiter)
                        .filter(|&s| !s.is_empty() && !s.contains("\0\0"))
                        .for_each(|s| {
                            let packet = Packet {
                                time: Instant::now().duration_since(t_zero).as_millis(),
                                direction: SerialDirection::Receive,
                                payload: s.to_owned(),
                            };
                            write_guard.push(packet)
                        });
                }
            }

            //std::thread::sleep(Duration::from_millis(10));
        }
        std::mem::drop(port);
    }
}
