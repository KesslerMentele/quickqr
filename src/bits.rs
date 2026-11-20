use bitvec::prelude::*;
use crate::EncodeDataError;

/// Appends the low `bit_length` bits of `value` onto `bits`.
/// `value` must already fit in `bit_length` bits.
///
/// # Errors
///
/// If the provided value does not fit in the provided bit_length.
///
/// If the provided bit length is invalid.
pub(crate) fn write_bits(bits: &mut BitVec<u8, Lsb0>, value: u32, bit_length: usize) -> Result<(), EncodeDataError > {
    if !(1..=32).contains(&bit_length) {
        return Err(EncodeDataError::InvalidBitLength(bit_length))
    }
    if bit_length < 32 && value >= (1u32 << bit_length) {
        return Err(EncodeDataError::BitsWouldOverflow { bit_length: value , bits_left: bit_length});
    }
    bits.extend((0..bit_length).rev().map(|i| (value >> i) & 1 == 1));
    Ok(())
}

/// Packs a byte-aligned, MSB-first-per-byte bitstream into bytes.
pub(crate) fn bits_to_bytes(bits: &BitSlice<u8, Lsb0>) -> Vec<u8> {
    debug_assert_eq!(bits.len() % 8, 0, "bits must be byte-aligned");
    bits.chunks(8).map(|chunk| chunk.iter().fold(0u8, |acc, bit| (acc << 1) | *bit as u8)).collect()
}

/// The inverse of `bits_to_bytes`: appends `bytes` onto a fresh bitstream.
pub(crate) fn bytes_to_bits(bytes: &[u8]) -> BitVec<u8, Lsb0> {
    let mut bits = BitVec::with_capacity(bytes.len() * 8);
    for &byte in bytes {
        write_bits(&mut bits, byte as u32, 8).expect("u8 did not fit in 8 bits...");
    }
    bits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_bits_appends_low_bits_msb_first() {
        let mut bits = BitVec::new();
        write_bits(&mut bits, 0b101, 3).unwrap();
        assert_eq!(bits, bits![1, 0, 1]);
    }

    #[test]
    fn write_bits_pads_to_the_requested_length() {
        let mut bits = BitVec::new();
        write_bits(&mut bits, 1, 4).unwrap();
        assert_eq!(bits, bits![0, 0, 0, 1]);
    }

    #[test]
    fn write_bits_accumulates_across_calls() {
        let mut bits = BitVec::new();
        write_bits(&mut bits, 0b11, 2).unwrap();
        write_bits(&mut bits, 0b0, 1).unwrap();
        write_bits(&mut bits, 0b101, 3).unwrap();
        assert_eq!(bits, bits![1, 1, 0, 1, 0, 1]);
    }

    #[test]
    #[should_panic]
    fn write_bits_rejects_a_value_that_does_not_fit() {
        let mut bits = BitVec::new();
        write_bits(&mut bits, 0b1000, 3).unwrap();
    }

    #[test]
    fn bytes_to_bits_and_back_round_trips() {
        let original = [0x10, 0x20, 0xEC, 0x00, 0xFF];
        assert_eq!(bits_to_bytes(&bytes_to_bits(&original)), original);
    }

    #[test]
    fn bytes_to_bits_is_msb_first_per_byte() {
        assert_eq!(bytes_to_bits(&[0b10110010]), bits![1, 0, 1, 1, 0, 0, 1, 0]);
    }
}
