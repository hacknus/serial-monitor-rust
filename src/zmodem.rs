use crossbeam_channel::Sender;
use serialport::SerialPort;
use std::ffi::OsString;
use std::fs::File;
use std::io::{BufReader, ErrorKind, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use zmodem2::{Action, Event, FileInfo, Position, Receiver, Sender as ZSender};

use crate::data::{get_epoch_ms, Packet, SerialDirection};

/// Wait time in seconds before requeuing a handshake.
const STALL_SECS: u64 = 5;

/// Size of a scratch buffer.
const WIRE_SCRATCH: usize = 4096;

type Port = BufReader<Box<dyn SerialPort>>;

#[derive(Debug, Clone)]
pub enum TransferCommand {
    Upload(PathBuf),
    Download(PathBuf),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TransferStatus {
    Active,
    Completed,
    Failed(String),
    Aborted,
}

#[derive(Debug, Clone)]
pub struct TransferProgress {
    pub direction: SerialDirection,
    pub filename: String,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub status: TransferStatus,
}

pub type ProgressLock = Arc<RwLock<Option<TransferProgress>>>;

enum End {
    Completed,
    Aborted,
}

fn publish(progress: &ProgressLock, prog: &TransferProgress) {
    if let Ok(mut guard) = progress.write() {
        *guard = Some(prog.clone());
    }
}

fn status(raw_data_tx: &Sender<Packet>, t_zero: Instant, msg: String) {
    let _ = raw_data_tx.send(Packet {
        relative_time: t_zero.elapsed().as_millis() as f64,
        absolute_time: get_epoch_ms() as f64,
        direction: SerialDirection::Receive,
        payload: msg,
    });
}

struct WireBuf {
    data: Vec<u8>,
    off: usize,
}

impl WireBuf {
    fn new() -> Self {
        Self {
            data: Vec::new(),
            off: 0,
        }
    }

    fn pending(&self) -> &[u8] {
        &self.data[self.off..]
    }

    fn advance(&mut self, n: usize) {
        self.off += n;
        if self.off >= self.data.len() {
            self.data.clear();
            self.off = 0;
        }
    }

    /// Reads more bytes from the port, returning the number of bytes read. A
    /// timeout reports `Ok(0)`; other I/O errors are propagated.
    fn fill(&mut self, port: &mut Port) -> Result<usize, String> {
        if self.off > 0 {
            self.data.drain(..self.off);
            self.off = 0;
        }
        let mut scratch = [0u8; WIRE_SCRATCH];
        match port.read(&mut scratch) {
            Ok(n) => {
                self.data.extend_from_slice(&scratch[..n]);
                Ok(n)
            }
            Err(ref e) if e.kind() == ErrorKind::TimedOut => Ok(0),
            Err(e) => Err(format!("serial read failed: {e}")),
        }
    }
}

fn write_wire(port: &mut Port, bytes: &[u8]) -> Result<usize, String> {
    match port.get_mut().write(bytes) {
        Ok(n) => Ok(n),
        Err(ref e) if e.kind() == ErrorKind::TimedOut => Ok(0),
        Err(e) => Err(format!("serial write failed: {e}")),
    }
}

fn sanitize_name(raw: &[u8]) -> OsString {
    let lossy = String::from_utf8_lossy(raw);
    match Path::new(lossy.as_ref()).file_name() {
        Some(name) if !name.is_empty() => name.to_os_string(),
        _ => OsString::from("received.bin"),
    }
}

pub fn upload(
    port: &mut Port,
    path: PathBuf,
    cancel: &Arc<AtomicBool>,
    progress: &ProgressLock,
    raw_data_tx: &Sender<Packet>,
    t_zero: Instant,
) {
    let filename = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".to_string());
    let outcome = upload_loop(
        port,
        &path,
        &filename,
        cancel,
        progress,
        raw_data_tx,
        t_zero,
    );
    finalize(
        &filename,
        SerialDirection::Send,
        outcome,
        progress,
        raw_data_tx,
        t_zero,
    );
}

fn upload_loop(
    port: &mut Port,
    path: &Path,
    filename: &str,
    cancel: &Arc<AtomicBool>,
    progress: &ProgressLock,
    raw_data_tx: &Sender<Packet>,
    t_zero: Instant,
) -> Result<End, String> {
    let mut file = File::open(path).map_err(|e| format!("cannot open {}: {e}", path.display()))?;
    let total = file
        .metadata()
        .map_err(|e| format!("cannot stat {}: {e}", path.display()))?
        .len();
    let size =
        u32::try_from(total).map_err(|_| "file too large for ZMODEM (> 4 GiB)".to_string())?;

    let mut sender = ZSender::new().map_err(|e| format!("zmodem init failed: {e:?}"))?;
    let name_bytes = filename.as_bytes();
    sender
        .start_file(FileInfo::new(name_bytes, Some(Position::new(size))))
        .map_err(|e| format!("start_file failed: {e:?}"))?;

    let mut prog = TransferProgress {
        direction: SerialDirection::Send,
        filename: filename.to_string(),
        bytes_done: 0,
        bytes_total: total,
        status: TransferStatus::Active,
    };
    publish(progress, &prog);
    status(
        raw_data_tx,
        t_zero,
        format!("ZMODEM: sending {filename} ({total} bytes)"),
    );

    let mut input = WireBuf::new();
    let mut fbuf: Vec<u8> = Vec::new();
    let mut last_activity = Instant::now();

    loop {
        if cancel.load(Ordering::SeqCst) {
            sender.abort();
            drain_upload(&mut sender, port);
            return Ok(End::Aborted);
        }

        let event = match sender.poll() {
            Action::WriteWire(bytes) => {
                let n = write_wire(port, bytes)?;
                sender.wire_written(n);
                continue;
            }
            Action::ReadFile { offset, max_len } => {
                let pos = u64::from(offset.get());
                file.seek(SeekFrom::Start(pos))
                    .map_err(|e| format!("seek failed: {e}"))?;
                if fbuf.len() < max_len {
                    fbuf.resize(max_len, 0);
                }
                let n = file
                    .read(&mut fbuf[..max_len])
                    .map_err(|e| format!("file read failed: {e}"))?;
                if n == 0 {
                    return Err("unexpected end of file".to_string());
                }
                sender
                    .submit_file(&fbuf[..n])
                    .map_err(|e| format!("submit_file failed: {e:?}"))?;
                prog.bytes_done = pos + n as u64;
                publish(progress, &prog);
                last_activity = Instant::now();
                continue;
            }
            Action::WriteFile(_) => continue,
            Action::Event(Event::FileCompleted) => SenderEvent::FileDone,
            Action::Event(Event::SessionCompleted) => SenderEvent::SessionDone,
            Action::Event(Event::Aborted) => SenderEvent::Aborted,
            Action::Event(_) => SenderEvent::Other,
            Action::Idle => {
                feed_wire(&mut input, port, &mut last_activity, || {
                    sender
                        .timeout()
                        .map_err(|e| format!("timeout failed: {e:?}"))
                })?;
                let pending = input.pending();
                if !pending.is_empty() {
                    let consumed = sender
                        .submit_wire(pending)
                        .map_err(|e| format!("protocol error: {e:?}"))?;
                    input.advance(consumed);
                }
                continue;
            }
            _ => continue,
        };

        match event {
            SenderEvent::FileDone => {
                sender
                    .finish()
                    .map_err(|e| format!("finish failed: {e:?}"))?;
            }
            SenderEvent::SessionDone => return Ok(End::Completed),
            SenderEvent::Aborted => return Ok(End::Aborted),
            SenderEvent::Other => {}
        }
    }
}

enum SenderEvent {
    FileDone,
    SessionDone,
    Aborted,
    Other,
}

pub fn download(
    port: &mut Port,
    dest_dir: PathBuf,
    cancel: &Arc<AtomicBool>,
    progress: &ProgressLock,
    raw_data_tx: &Sender<Packet>,
    t_zero: Instant,
) {
    let outcome = download_loop(port, &dest_dir, cancel, progress, raw_data_tx, t_zero);
    let filename = progress
        .read()
        .ok()
        .and_then(|g| g.as_ref().map(|p| p.filename.clone()))
        .unwrap_or_default();
    finalize(
        &filename,
        SerialDirection::Receive,
        outcome,
        progress,
        raw_data_tx,
        t_zero,
    );
}

fn download_loop(
    port: &mut Port,
    dest_dir: &Path,
    cancel: &Arc<AtomicBool>,
    progress: &ProgressLock,
    raw_data_tx: &Sender<Packet>,
    t_zero: Instant,
) -> Result<End, String> {
    let mut receiver = Receiver::new().map_err(|e| format!("zmodem init failed: {e:?}"))?;

    let mut prog = TransferProgress {
        direction: SerialDirection::Receive,
        filename: String::new(),
        bytes_done: 0,
        bytes_total: 0,
        status: TransferStatus::Active,
    };
    publish(progress, &prog);
    status(
        raw_data_tx,
        t_zero,
        format!("ZMODEM: waiting for file into {}", dest_dir.display()),
    );

    let mut input = WireBuf::new();
    let mut current: Option<File> = None;
    let mut last_activity = Instant::now();

    loop {
        if cancel.load(Ordering::SeqCst) {
            let _ = receiver.abort();
            drain_download(&mut receiver, port);
            return Ok(End::Aborted);
        }

        let event = match receiver.poll() {
            Action::WriteWire(bytes) => {
                let n = write_wire(port, bytes)?;
                receiver.wire_written(n);
                continue;
            }
            Action::WriteFile(bytes) => {
                let file = current
                    .as_mut()
                    .ok_or_else(|| "received data before file start".to_string())?;
                let n = file
                    .write(bytes)
                    .map_err(|e| format!("file write failed: {e}"))?;
                receiver
                    .file_written(n)
                    .map_err(|e| format!("file_written failed: {e:?}"))?;
                prog.bytes_done += n as u64;
                publish(progress, &prog);
                last_activity = Instant::now();
                continue;
            }
            Action::ReadFile { .. } => continue,
            Action::Event(Event::FileStarted(info)) => {
                let name = sanitize_name(info.name);
                let size = info.size.map_or(0, |p| u64::from(p.get()));
                ReceiverEvent::Started { name, size }
            }
            Action::Event(Event::FileCompleted) => ReceiverEvent::FileDone,
            Action::Event(Event::SessionCompleted) => ReceiverEvent::SessionDone,
            Action::Event(Event::Aborted) => ReceiverEvent::Aborted,
            Action::Idle => {
                feed_wire(&mut input, port, &mut last_activity, || {
                    receiver
                        .timeout()
                        .map_err(|e| format!("timeout failed: {e:?}"))
                })?;
                let pending = input.pending();
                if !pending.is_empty() {
                    let consumed = receiver
                        .submit_wire(pending)
                        .map_err(|e| format!("protocol error: {e:?}"))?;
                    input.advance(consumed);
                }
                continue;
            }
            _ => continue,
        };

        match event {
            ReceiverEvent::Started { name, size } => {
                let path = dest_dir.join(&name);
                let file = File::create(&path)
                    .map_err(|e| format!("cannot create {}: {e}", path.display()))?;
                current = Some(file);
                prog.filename = name.to_string_lossy().into_owned();
                prog.bytes_done = 0;
                prog.bytes_total = size;
                publish(progress, &prog);
                status(
                    raw_data_tx,
                    t_zero,
                    format!("ZMODEM: receiving {} ({size} bytes)", prog.filename),
                );
            }
            ReceiverEvent::FileDone => {
                if let Some(mut file) = current.take() {
                    let _ = file.flush();
                }
                status(
                    raw_data_tx,
                    t_zero,
                    format!("ZMODEM: received {}", prog.filename),
                );
            }
            ReceiverEvent::SessionDone => return Ok(End::Completed),
            ReceiverEvent::Aborted => return Ok(End::Aborted),
        }
    }
}

enum ReceiverEvent {
    Started { name: OsString, size: u64 },
    FileDone,
    SessionDone,
    Aborted,
}

fn feed_wire(
    input: &mut WireBuf,
    port: &mut Port,
    last_activity: &mut Instant,
    nudge: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    if !input.pending().is_empty() {
        return Ok(());
    }
    let n = input.fill(port)?;
    if n > 0 {
        *last_activity = Instant::now();
    } else if last_activity.elapsed() > Duration::from_secs(STALL_SECS) {
        nudge()?;
        *last_activity = Instant::now();
    }
    Ok(())
}

fn drain_upload(sender: &mut ZSender, port: &mut Port) {
    while let Action::WriteWire(bytes) = sender.poll() {
        match write_wire(port, bytes) {
            Ok(0) | Err(_) => break,
            Ok(n) => sender.wire_written(n),
        }
    }
}

fn drain_download(receiver: &mut Receiver, port: &mut Port) {
    while let Action::WriteWire(bytes) = receiver.poll() {
        match write_wire(port, bytes) {
            Ok(0) | Err(_) => break,
            Ok(n) => receiver.wire_written(n),
        }
    }
}

fn finalize(
    filename: &str,
    direction: SerialDirection,
    outcome: Result<End, String>,
    progress: &ProgressLock,
    raw_data_tx: &Sender<Packet>,
    t_zero: Instant,
) {
    let (status_value, msg) = match outcome {
        Ok(End::Completed) => (
            TransferStatus::Completed,
            format!("ZMODEM: transfer of {filename} completed"),
        ),
        Ok(End::Aborted) => (
            TransferStatus::Aborted,
            format!("ZMODEM: transfer of {filename} aborted"),
        ),
        Err(e) => (
            TransferStatus::Failed(e.clone()),
            format!("ZMODEM: transfer of {filename} failed: {e}"),
        ),
    };

    let (bytes_done, bytes_total) = progress
        .read()
        .ok()
        .and_then(|g| g.as_ref().map(|p| (p.bytes_done, p.bytes_total)))
        .unwrap_or((0, 0));
    if let Ok(mut guard) = progress.write() {
        *guard = Some(TransferProgress {
            direction,
            filename: filename.to_string(),
            bytes_done,
            bytes_total,
            status: status_value,
        });
    }
    status(raw_data_tx, t_zero, msg);
}
