use std::{io, time};

pub struct Stdio;

impl serialport::SerialPort for Stdio {
    fn name(&self) -> Option<String> {
        todo!()
    }

    fn baud_rate(&self) -> serialport::Result<u32> {
        todo!()
    }

    fn data_bits(&self) -> serialport::Result<serialport::DataBits> {
        todo!()
    }

    fn flow_control(&self) -> serialport::Result<serialport::FlowControl> {
        todo!()
    }

    fn parity(&self) -> serialport::Result<serialport::Parity> {
        todo!()
    }

    fn stop_bits(&self) -> serialport::Result<serialport::StopBits> {
        todo!()
    }

    fn timeout(&self) -> time::Duration {
        todo!()
    }

    fn set_baud_rate(&mut self, _baud_rate: u32) -> serialport::Result<()> {
        todo!()
    }

    fn set_data_bits(&mut self, _data_bits: serialport::DataBits) -> serialport::Result<()> {
        todo!()
    }

    fn set_flow_control(
        &mut self,
        _flow_control: serialport::FlowControl,
    ) -> serialport::Result<()> {
        todo!()
    }

    fn set_parity(&mut self, _parity: serialport::Parity) -> serialport::Result<()> {
        todo!()
    }

    fn set_stop_bits(&mut self, _stop_bits: serialport::StopBits) -> serialport::Result<()> {
        todo!()
    }

    fn set_timeout(&mut self, _timeout: time::Duration) -> serialport::Result<()> {
        todo!()
    }

    fn write_request_to_send(&mut self, _level: bool) -> serialport::Result<()> {
        todo!()
    }

    fn write_data_terminal_ready(&mut self, _level: bool) -> serialport::Result<()> {
        todo!()
    }

    fn read_clear_to_send(&mut self) -> serialport::Result<bool> {
        todo!()
    }

    fn read_data_set_ready(&mut self) -> serialport::Result<bool> {
        todo!()
    }

    fn read_ring_indicator(&mut self) -> serialport::Result<bool> {
        todo!()
    }

    fn read_carrier_detect(&mut self) -> serialport::Result<bool> {
        todo!()
    }

    fn bytes_to_read(&self) -> serialport::Result<u32> {
        todo!()
    }

    fn bytes_to_write(&self) -> serialport::Result<u32> {
        todo!()
    }

    fn clear(&self, _buffer_to_clear: serialport::ClearBuffer) -> serialport::Result<()> {
        todo!()
    }

    fn try_clone(&self) -> serialport::Result<Box<dyn serialport::SerialPort>> {
        todo!()
    }

    fn set_break(&self) -> serialport::Result<()> {
        todo!()
    }

    fn clear_break(&self) -> serialport::Result<()> {
        todo!()
    }
}

impl io::Write for Stdio {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        io::stdout().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        io::stdout().flush()
    }
}

impl io::Read for Stdio {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        io::stdin().read(buf)
    }
}
