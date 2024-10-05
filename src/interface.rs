use std::io;

pub enum Interface {
    SerialPort(Box<dyn serialport::SerialPort>),
    Stdio,
}

impl io::Write for Interface {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Interface::SerialPort(s) => s.write(buf),
            Interface::Stdio => io::stdout().write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Interface::SerialPort(s) => s.flush(),
            Interface::Stdio => io::stdout().flush(),
        }
    }
}

impl io::Read for Interface {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Interface::SerialPort(s) => s.read(buf),
            Interface::Stdio => io::stdin().read(buf),
        }
    }
}
