//! Compile-time IEEE 802.3 CRC-32 polynomial hashing over GF(2) for structural fingerprinting.

pub const CRC32_POLYNOMIAL: u32 = 0xEDB8_8320;
pub const MAX_CRC_SCAN_BYTES: usize = 16_384;

pub const fn make_crc_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0usize;
    while i < 256 {
        let mut crc = i as u32;
        let mut j = 0usize;
        while j < 8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (CRC32_POLYNOMIAL & mask);
            j += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
}

pub static CRC_TABLE: [u32; 256] = make_crc_table();

#[inline]
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    let limit = match data.len() > MAX_CRC_SCAN_BYTES {
        true => MAX_CRC_SCAN_BYTES,
        false => data.len(),
    };
    let mut i = 0usize;
    while i < limit {
        let byte = data[i];
        let index = ((crc ^ (byte as u32)) & 0xFF) as usize;
        crc = (crc >> 8) ^ CRC_TABLE[index];
        i += 1;
    }
    crc ^ 0xFFFF_FFFFu32
}

#[inline]
pub fn calculate_crc32(data: &[u8]) -> u32 {
    crc32(data)
}

#[inline]
pub fn compute_crc32(data: &[u8]) -> u32 {
    crc32(data)
}