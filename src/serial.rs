use crate::data::SerialDirection;
use crate::Device;
use crate::{print_to_console, Packet, Print};
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
    send_rx: Receiver<String>,
    device_lock: Arc<RwLock<Device>>,
    devices_lock: Arc<RwLock<Vec<String>>>,
    raw_data_lock: Arc<RwLock<Vec<Packet>>>,
    print_lock: Arc<RwLock<Vec<Print>>>,
    connected_lock: Arc<RwLock<bool>>,
) {
    loop {
        let _not_awake = keepawake::Builder::new()
            .display(false)
            .reason("Serial Connection")
            .app_name("Serial Monitor")
            //.app_reverse_domain("io.github.myprog")
            .create()
            .unwrap();

        if let Ok(mut connected) = connected_lock.write() {
            *connected = false;
        }

        let device = get_device(devices_lock.clone(), device_lock.clone());

        let mut port = match serialport::new(&device.name, device.baud_rate)
            .timeout(Duration::from_millis(100))
            .open()
        {
            Ok(p) => {
                if let Ok(mut connected) = connected_lock.write() {
                    *connected = true;
                }
                BufReader::new(p)
            }
            Err(err) => {
                if let Ok(mut write_guard) = device_lock.write() {
                    write_guard.name.clear();
                }
                print_to_console(
                    &print_lock,
                    Print::Error(format!("Error connecting: {}", err)),
                );
                continue;
            }
        };

        let t_zero = Instant::now();

        print_to_console(
            &print_lock,
            Print::Ok(format!(
                "Connected to serial port: {} @ baud = {}",
                device.name, device.baud_rate
            )),
        );

        let _awake = keepawake::Builder::new()
            .display(true)
            .reason("Serial Connection")
            .app_name("Serial Monitor")
            //.app_reverse_domain("io.github.myprog")
            .create()
            .unwrap();

        'connected_loop: loop {
            let devices: Vec<String> = serialport::available_ports()
                .unwrap()
                .iter()
                .map(|p| p.port_name.clone())
                .collect();

            if let Ok(mut write_guard) = devices_lock.write() {
                *write_guard = devices.clone();
            }

            if let Ok(read_guard) = device_lock.read() {
                if device.name != read_guard.name {
                    print_to_console(
                        &print_lock,
                        Print::Ok(format!("Disconnected from serial port: {}", device.name)),
                    );
                    break 'connected_loop;
                }
            }

            if !devices.contains(&device.name) {
                print_to_console(
                    &print_lock,
                    Print::Error(format!(
                        "Device has disconnected from serial port: {}",
                        device.name
                    )),
                );
                if let Ok(mut write_guard) = device_lock.write() {
                    write_guard.name.clear();
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

fn get_device(devices_lock: Arc<RwLock<Vec<String>>>, d_lock: Arc<RwLock<Device>>) -> Device {
    loop {
        let devices: Vec<String> = serialport::available_ports()
            .unwrap()
            .iter()
            .map(|p| p.port_name.clone())
            .collect();

        if let Ok(mut write_guard) = devices_lock.write() {
            *write_guard = devices.clone();
        }

        if let Ok(device) = d_lock.read() {
            if devices.contains(&device.name) {
                return Device {
                    name: device.name.clone(),
                    baud_rate: device.baud_rate,
                };
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
