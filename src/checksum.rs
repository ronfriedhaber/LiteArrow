/// Computes CRC-32C (Castagnoli), the checksum used by LiteArrow metadata and
/// column chunks.
///
/// This deliberately small implementation favors clarity. It can later be
/// replaced by a hardware-accelerated implementation without changing the
/// bytes stored in the format.
pub fn crc32c(bytes: &[u8]) -> u32 {
    const REVERSED_CASTAGNOLI_POLYNOMIAL: u32 = 0x82f6_3b78;

    let mut crc = !0_u32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let low_bit_mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (REVERSED_CASTAGNOLI_POLYNOMIAL & low_bit_mask);
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_standard_crc32c_check_value() {
        assert_eq!(crc32c(b"123456789"), 0xe306_9283);
    }
}
