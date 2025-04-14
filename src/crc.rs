pub fn calculate_crc(data: &str) -> u8 {
    let polynomial: u8 = 0x07;
    let mut crc: u8 = 0x00;

    for byte in data.bytes() {
        crc ^= byte;
        for _ in 0..8 {
            if crc & 0x80 != 0 {
                crc = (crc << 1) ^ polynomial;
            } else {
                crc <<= 1;
            }
        }
    }

    crc
}
