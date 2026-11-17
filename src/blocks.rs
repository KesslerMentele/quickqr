use bitvec::prelude::*;
use crate::bits::bits_to_bytes;
use crate::capacity::{ec_block_info, EcBlockInfo};
use crate::gf_math::{error_correction_codewords, EcLength};
use crate::qr_code::ErrorCorrectionLevel;

/// Slices `data` into its error-correction blocks.
fn split_into_blocks(data: &[u8], info: EcBlockInfo) -> Vec<&[u8]> {
    let mut blocks = Vec::with_capacity((info.group_1_blocks + info.group_2_blocks) as usize);
    let mut rest = data;
    for _ in 0..info.group_1_blocks {
        let (block, remainder) = rest.split_at(info.group_1_data_codewords as usize);
        blocks.push(block);
        rest = remainder;
    }
    for _ in 0..info.group_2_blocks {
        let (block, remainder) = rest.split_at(info.group_2_data_codewords as usize);
        blocks.push(block);
        rest = remainder;
    }
    blocks
}


/// Builds the final, interleaved codeword sequence ready for placement.
pub(crate) fn interleave(data: &BitSlice<u8, Lsb0>, version: u8, error_correction_level: ErrorCorrectionLevel) -> BitVec<u8, Lsb0> {
    let data = bits_to_bytes(data);

    let info = ec_block_info(version, error_correction_level);
    let ec_codewords_per_block = info.ec_codewords_per_block as usize;
    let ec_len = EcLength::try_from(info.ec_codewords_per_block as u8)
        .expect("ec_codewords_per_block is always a valid EcLength"); // This can never fail

    let blocks = split_into_blocks(&data, info);
    let error_correction: Vec<u8> = blocks.iter().flat_map(|block| error_correction_codewords(block, ec_len) ).collect();

    let total_bytes = data.len() + error_correction.len();
    let mut result = BitVec::with_capacity(total_bytes * 8);

    let max_data_len = blocks.iter().map(|block| block.len()).max().unwrap_or(0);
    for i in 0..max_data_len {
        for block in &blocks {
            if let Some(&byte) = block.get(i) {
                result.extend_from_bitslice(byte.view_bits::<Msb0>());
            }
        }
    }
    for i in 0..ec_codewords_per_block {
        for block_index in 0..blocks.len() {
            let byte = error_correction[block_index * ec_codewords_per_block + i];
            result.extend_from_bitslice(byte.view_bits::<Msb0>());
        }
    }

    result
}

#[cfg(test)]
/// The inverse of `interleave`, for a symbol's codewords: one `data + error_correction` sequence
/// per block.
pub(crate) fn deinterleave(codewords: &[u8], info: EcBlockInfo) -> Vec<Vec<u8>> {
    let data_lengths: Vec<usize> = std::iter::repeat_n(info.group_1_data_codewords as usize, info.group_1_blocks as usize)
        .chain(std::iter::repeat_n(info.group_2_data_codewords as usize, info.group_2_blocks as usize))
        .collect();
    let mut blocks: Vec<Vec<u8>> = data_lengths.iter().map(|&length| Vec::with_capacity(length)).collect();
    let mut remaining = codewords.iter();
    let mut next = || remaining.next().expect("a symbol holds every codeword of its blocks");

    let max_data_len = *data_lengths.iter().max().unwrap_or(&0);
    for i in 0..max_data_len {
        for (block, &length) in blocks.iter_mut().zip(&data_lengths) {
            if i < length {
                block.push(*next());
            }
        }
    }
    for _ in 0..info.ec_codewords_per_block {
        for block in &mut blocks {
            block.push(*next());
        }
    }
    blocks
}

#[cfg(test)]
mod tests {
    use crate::bits::bytes_to_bits;
    use crate::capacity::data_codewords;
    use super::*;

    #[test]
    fn deinterleave_undoes_interleave_across_unequal_blocks() {
        // 5-Q: two blocks of 15 data codewords, then two of 16, each with 18 EC codewords.
        let version = 5;
        let level = ErrorCorrectionLevel::Q;
        let info = ec_block_info(version, level);
        let data: Vec<u8> = (0..data_codewords(version, level) as u8).collect();

        let interleaved = bits_to_bytes(&interleave(&bytes_to_bits(&data), version, level));
        let blocks = deinterleave(&interleaved, info);

        assert_eq!(blocks.iter().map(Vec::len).collect::<Vec<_>>(), [33, 33, 34, 34]);
        let recovered: Vec<u8> = blocks.iter().zip([15, 15, 16, 16]).flat_map(|(block, length)| block[..length].to_vec()).collect();
        assert_eq!(recovered, data);
    }
}
