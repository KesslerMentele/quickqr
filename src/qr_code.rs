use std::fmt;
use bitvec::prelude::*;
#[cfg(feature = "serde")]
use serde::Deserialize;
use crate::qr_code::{
    bits::bytes_to_bits,
    blocks::interleave,
    structure::{QrMatrix, MaskError},
    encoder::{apply_formatting, encode_data_by_encoding_plan, encode_single_segment, EncodeError},
    segmenter::{best_single_mode, segment, SegmentError, SegmentMode},
};


pub mod penalty_score;
pub mod gf_math;
pub mod encoder;
pub mod decoder;
pub mod bits;
pub mod segmenter;
mod capacity;
mod structure;
pub mod blocks;
pub(crate) mod prelude;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum QrVersionError {
    OutOfBounds(u8)
}
impl fmt::Display for QrVersionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self { QrVersionError::OutOfBounds(version) => {write!(f, "QR version out of bounds: {}", version)} }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum QrVersion {
    Auto,
    #[cfg_attr(feature = "serde",  serde(untagged))]
    Exact(#[cfg_attr(feature = "serde", serde(deserialize_with = "u8_from_str"))] u8),
}
impl fmt::Display for QrVersion {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            QrVersion::Auto => { write!(f, "QRVersion::Auto") }
            QrVersion::Exact(version) => {write!(f, "QRVersion::Exact({})", version)}
        }
    }
}
impl TryFrom<u8> for QrVersion {
    type Error = QrVersionError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if value >= 1 && value <= 40 {Ok(QrVersion::Exact(value))} else {Err(QrVersionError::OutOfBounds(value)) }
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum EncodingMode {
    Numeric,
    Alphanumeric,
    Byte,
    Kanji,
    Eci,
    Fnc1,
    StructuredAppend,
    Auto,
    Segmented
}
impl TryFrom<SegmentMode> for EncodingMode {
    type Error = QrCreationError;

    fn try_from(value: SegmentMode) -> Result<Self, QrCreationError> {
        match value {
            SegmentMode::Numeric => {Ok(EncodingMode::Numeric)}
            SegmentMode::Alphanumeric => { Ok(EncodingMode::Alphanumeric)}
            SegmentMode::Byte => {Ok(EncodingMode::Byte)}

            SegmentMode::Kanji => {
                if cfg!(feature = "kanji") {
                    Ok(EncodingMode::Kanji)

                } else {
                    Err(QrCreationError::FeatureNotEnabled("kanji"))
                }

            }
        }
    }
}
impl fmt::Display for EncodingMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ErrorCorrectionLevel {
    L,
    M,
    Q,
    H
}
 impl fmt::Display for ErrorCorrectionLevel {
     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
         write!(f, "{:?}", self)
     }
 }

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QrCreationError {
    InvalidQRVersion(QrVersionError),
    NotSegmentable(SegmentError),
    NotImplemented(EncodingMode),
    EncodingFailed(EncodeError),
    MaskingFailed(MaskError),
    FeatureNotEnabled(&'static str)
}
impl From<SegmentError> for QrCreationError {
    fn from(value: SegmentError) -> Self { QrCreationError::NotSegmentable(value) }
}
impl From<QrVersionError> for QrCreationError {
    fn from(value: QrVersionError) -> Self { QrCreationError::InvalidQRVersion(value) }
}
impl From<EncodeError> for QrCreationError {
    fn from(value: EncodeError) -> Self { QrCreationError::EncodingFailed(value) }
}
impl fmt::Display for QrCreationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            QrCreationError::InvalidQRVersion(version_error) => {
                write!(f, "Invalid QR version. Failed with error: {}", version_error)
            }
            QrCreationError::NotSegmentable(segment_error) => {
                write!(f, "QR version is not segmentable. Failed with error: {}", segment_error)
            }
            QrCreationError::NotImplemented(mode) => {
                write!(f, "requested encoding mode is not implemented: {}", mode)
            }
            QrCreationError::EncodingFailed(encode_error) => {
                write!(f, "QR encoding failed with error: {}", encode_error)
            }
            QrCreationError::MaskingFailed(mask_error) => {
                write!(f, "QR masking failed with error: {}", mask_error)
            }
            QrCreationError::FeatureNotEnabled(feature_name) => {
                write!(f, "Feature not enabled: {}", feature_name)
            }
        }
    }
}


/// Encoded segment bits (headers and data, no terminator or padding yet), and the version they
/// were sized for.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EncodedData {
    pub(crate) version: u8,
    pub(crate) bits: BitVec<u8, Lsb0>,
    /// Where each segment landed in `bits`, in order.
    pub(crate) segments: Vec<SegmentStats>,
}
impl fmt::Display for EncodedData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "version: {}, bits: {:?}, segments: {:?}", self.version, self.bits, self.segments )
    }
}

/// Where one segment sits in the encoded data, as bit offsets from the start of the data stream.
/// These are offsets before interleaving, so with more than one error correction block a segment's
/// bits aren't contiguous in the finished symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct SegmentStats {
    pub(crate) mode: SegmentMode,
    /// The first bit of the segment's header.
    pub(crate) start_bit: usize,
    /// One past the segment's last bit, which is where the next segment starts.
    pub(crate) end_bit: usize,
    /// The mode indicator and character count: the segment's data starts at `start_bit + header_bits`.
    pub(crate) header_bits: usize,
}
impl fmt::Display for SegmentStats {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "mode:{}, start:{}, end:{}, header:{}", self.mode, self.start_bit, self.end_bit, self.header_bits)
    }
}

/// What was actually used to build a code, with every `Auto` setting resolved.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QrStats {
    pub(crate) version: u8,
    pub(crate) error_correction_level: ErrorCorrectionLevel,
    pub(crate) mask: u8,
    /// The mode of every segment, in order.
    pub(crate) encoding_modes: Vec<SegmentMode>,
    pub(crate) segments: Vec<SegmentStats>,
}
impl fmt::Display for QrStats {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "version: {}, error correction level: {}, mask: {},encoding modes: {:?}, segments: {:?}",
               self.version, self.error_correction_level, self.mask, self.encoding_modes, self.segments
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QrCode {
    pub matrix: QrMatrix,
    pub stats: QrStats,
}
impl fmt::Display for QrCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "matrix: {}, stats: {}", self.matrix, self.stats)
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) enum MaskSetting {
    Auto,
    #[cfg_attr(feature = "serde", serde(untagged))]
    Exact(#[cfg_attr(feature = "serde", serde(deserialize_with = "u8_from_str"))] u8),
}
impl fmt::Display for MaskSetting {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MaskSetting::Auto => {
                write!(f, "auto")
            }
            MaskSetting::Exact(val) => {
                write!(f, "exact({})", val)
            }
        }
    }
}


#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QrData {
    text: String,
    error_correction_level: ErrorCorrectionLevel,
    encoding_mode: EncodingMode,
    version: QrVersion,
    mask: MaskSetting
}
impl QrData {
    fn new(data: &str, error_correction_level: ErrorCorrectionLevel, encoding_mode: EncodingMode, version: QrVersion, mask: MaskSetting) -> QrData {
        QrData {
            text: data.to_string(),
            error_correction_level,
            encoding_mode,
            version,
            mask
        }
    }
}
impl fmt::Display for QrData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "text:{}, error correction level: {}, encoding mode: {}, version: {}, mask: {}",
               self.text, self.error_correction_level, self.encoding_mode, self.version, self.mask
        )
    }
}

#[cfg(feature="serde")]
fn u8_from_str<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u8, D::Error> {
    String::deserialize(d)?.parse().map_err(serde::de::Error::custom)
}

fn create_qr_code_bits(
    data: &QrData
) -> Result<EncodedData, QrCreationError> {
    
    let mode = match data.encoding_mode {
        EncodingMode::Segmented => {
            let segment_plan = segment(&*data.text, data.version, data.error_correction_level)?;
            return Ok(encode_data_by_encoding_plan(&*data.text, segment_plan)?);
        }
        EncodingMode::Auto => best_single_mode(&*data.text),
        EncodingMode::Numeric => SegmentMode::Numeric,
        EncodingMode::Alphanumeric => SegmentMode::Alphanumeric,
        EncodingMode::Byte => SegmentMode::Byte,
        #[cfg(feature = "kanji")]
        EncodingMode::Kanji => SegmentMode::Kanji,
        mode @ (EncodingMode::Eci | EncodingMode::Fnc1 | EncodingMode::StructuredAppend) => {
            return Err(QrCreationError::NotImplemented(mode));
        }
    };
    encode_single_segment(&*data.text, mode, data.version, data.error_correction_level)
}

fn create_error_corrected_data(encoded_data: EncodedData, error_correction_level:ErrorCorrectionLevel) -> BitVec<u8> {
    interleave(
        &bytes_to_bits(
            &apply_formatting(
                encoded_data.bits,
                encoded_data.version,
                error_correction_level)
        ),
        encoded_data.version, error_correction_level)
}

pub fn create_qr_code(data: QrData) -> Result<QrCode, QrCreationError> {
    let encoded_data = create_qr_code_bits(&data)?;
    let segments = encoded_data.segments.clone();
    let mut matrix = QrMatrix::new(encoded_data.version, data.error_correction_level);
    let error_corrected_data = create_error_corrected_data(encoded_data, data.error_correction_level);
    matrix.place_data(&error_corrected_data);
    matrix.set_mask(data.mask).map_err(QrCreationError::MaskingFailed)?;
    let stats = QrStats {
        version: matrix.version(),
        error_correction_level: data.error_correction_level,
        mask: matrix.mask().expect("set_mask succeeded, so a mask is set"),
        encoding_modes: segments.iter().map(|segment| segment.mode).collect(),
        segments,
    };
    Ok(QrCode { matrix, stats })
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::bits::write_bits;

    fn v(version: u8) -> QrVersion { QrVersion::Exact(version) }

    /// Builds an expected bitstream from `(value, bit_length)` fields, in order.
    fn fields(fields: &[(u32, usize)]) -> BitVec<u8, Lsb0> {
        let mut bits = BitVec::new();
        for &(value, bit_length) in fields {
            write_bits(&mut bits, value, bit_length);
        }
        bits
    }

    #[test]
    fn segmented_single_mode_matches_the_iso_18004_example() {
        // ISO/IEC 18004 Annex I: "01234567" at 1-M is mode 0001, count 8 in 10 bits, then data.
        let expected = fields(&[(0b0001, 4), (8, 10), (12, 10), (345, 10), (67, 7)]);
        let bits = create_qr_code_bits(&QrData::new("01234567", ErrorCorrectionLevel::M, EncodingMode::Segmented, v(1), MaskSetting::Auto)).unwrap().bits;
        assert_eq!(bits, expected);
    }

    #[test]
    fn segmented_writes_a_header_for_every_segment() {
        // Byte "a", then Numeric "1234567" (see segmenter's worked example).
        let expected = fields(&[
            (0b0100, 4), (1, 8), (b'a' as u32, 8),
            (0b0001, 4), (7, 10), (123, 10), (456, 10), (7, 4),
        ]);
        let bits = create_qr_code_bits(&QrData::new("a1234567", ErrorCorrectionLevel::L, EncodingMode::Segmented, QrVersion::Auto, MaskSetting::Auto)).unwrap().bits;
        assert_eq!(bits, expected);
    }

    #[test]
    fn segmented_length_matches_the_segment_plan() {
        for data in ["01234567", "a1234567", "AB1234567", "é1234567", "点点abc", "HELLO WORLD 123"] {
            let plan = segment(data, QrVersion::Auto, ErrorCorrectionLevel::M).unwrap();
            let bits = create_qr_code_bits(&QrData::new(data, ErrorCorrectionLevel::M, EncodingMode::Segmented, QrVersion::Auto, MaskSetting::Auto )).unwrap().bits;
            assert_eq!(bits.len(), plan.data_bits_length, "length mismatch for {data:?}");
        }
    }

    #[test]
    fn segmented_count_indicator_widens_with_the_version() {
        // Byte's count indicator is 8 bits up to version 9, 16 bits from version 10.
        let expected = fields(&[(0b0100, 4), (2, 16), (b'a' as u32, 8), (b'b' as u32, 8)]);
        let bits = create_qr_code_bits(&QrData::new("ab", ErrorCorrectionLevel::L, EncodingMode::Segmented, v(10), MaskSetting::Auto)).unwrap().bits;
        assert_eq!(bits, expected);
    }

    #[test]
    fn segmented_rejects_data_that_does_not_fit_a_fixed_version() {
        let result = create_qr_code_bits(&QrData::new(&"a".repeat(20), ErrorCorrectionLevel::L, EncodingMode::Segmented, v(1), MaskSetting::Auto));
        assert!(matches!(result, Err(QrCreationError::NotSegmentable(SegmentError::ExceedsVersionCapacity { .. }))));
    }

    #[test]
    fn auto_detect_matches_the_iso_18004_alphanumeric_example() {
        // ISO/IEC 18004 Annex I: "AC-42" is mode 0010, count 5 in 9 bits, then data.
        let expected = fields(&[(0b0010, 4), (5, 9), (10 * 45 + 12, 11), (41 * 45 + 4, 11), (2, 6)]);
        let bits = create_qr_code_bits(&QrData::new("AC-42", ErrorCorrectionLevel::M, EncodingMode::Auto, QrVersion::Auto, MaskSetting::Auto)).unwrap().bits;
        assert_eq!(bits, expected);
    }

    #[test]
    #[cfg(feature = "kanji")]
    fn kanji_count_indicator_counts_chars_not_bytes() {
        // 点 is 3 UTF-8 bytes, but the count is 1 char. Data is 0x935F reduced to 0x0D9F.
        let expected = fields(&[(0b1000, 4), (1, 8), (0x0D9F, 13)]);
        let bits = create_qr_code_bits(&QrData::new("点", ErrorCorrectionLevel::M, EncodingMode::Kanji, QrVersion::Auto, MaskSetting::Auto)).unwrap().bits;
        assert_eq!(bits, expected);
    }

    #[test]
    fn byte_count_indicator_counts_utf8_bytes() {
        // é is 2 UTF-8 bytes: 0xC3 0xA9.
        let expected = fields(&[(0b0100, 4), (2, 8), (0xC3, 8), (0xA9, 8)]);
        let bits = create_qr_code_bits(&QrData::new("é", ErrorCorrectionLevel::M, EncodingMode::Byte, QrVersion::Auto, MaskSetting::Auto)).unwrap().bits;
        assert_eq!(bits, expected);
    }

    #[test]
    fn fixed_mode_rejects_data_that_does_not_fit_a_fixed_version() {
        let result = create_qr_code_bits(&QrData::new(&"a".repeat(20), ErrorCorrectionLevel::L, EncodingMode::Byte, v(1), MaskSetting::Auto));
        assert!(result.is_err(), "20 bytes needs 172 bits but 1-L only holds 152");
    }

    #[test]
    fn auto_detect_widens_the_count_in_a_larger_version_group() {
        // 300 bytes overflows every Small version at L, so the count indicator is 16 bits.
        let bits = create_qr_code_bits(&QrData::new(&"a".repeat(300), ErrorCorrectionLevel::L, EncodingMode::Auto, QrVersion::Auto, MaskSetting::Auto)).unwrap().bits;
        assert_eq!(bits.len(), 4 + 16 + 300 * 8);
        assert_eq!(bits[..20], fields(&[(0b0100, 4), (300, 16)])[..]);
    }

    #[test]
    fn auto_detect_rejects_data_too_long_for_version_40() {
        let result = create_qr_code_bits(&QrData::new(&"a".repeat(3000), ErrorCorrectionLevel::L, EncodingMode::Auto, QrVersion::Auto, MaskSetting::Auto));
        assert!(matches!(result, Err(QrCreationError::NotSegmentable(SegmentError::TooLong { needed_bits: 24020 }))));
    }

    #[test]
    fn fixed_mode_rejects_chars_outside_its_set() {
        let result = create_qr_code_bits(&QrData::new("12a", ErrorCorrectionLevel::M, EncodingMode::Numeric, QrVersion::Auto, MaskSetting::Auto));
        assert!(matches!(result, Err(QrCreationError::EncodingFailed(EncodeError::InvalidChar('a')))));
    }

    #[test]
    fn prefix_modes_are_not_implemented() {
        for mode in [EncodingMode::Eci, EncodingMode::Fnc1, EncodingMode::StructuredAppend] {
            let result = create_qr_code_bits(&QrData::new("1", ErrorCorrectionLevel::M, mode, QrVersion::Auto, MaskSetting::Auto));
            assert!(matches!(result, Err(QrCreationError::NotImplemented(_))));
        }
    }

    #[test]
    fn segmented_empty_input_is_empty() {
        let bits = create_qr_code_bits(&QrData::new("", ErrorCorrectionLevel::M, EncodingMode::Segmented, QrVersion::Auto, MaskSetting::Auto)).unwrap().bits;
        assert!(bits.is_empty());
    }

    fn stats(text: &str, encoding_mode: EncodingMode, mask: MaskSetting) -> QrStats {
        create_qr_code(QrData::new(text, ErrorCorrectionLevel::Q, encoding_mode, QrVersion::Auto, mask)).unwrap().stats
    }

    #[test]
    fn stats_report_the_version_and_level_used() {
        let long = stats(&"a".repeat(300), EncodingMode::Byte, MaskSetting::Auto);
        assert_eq!(long.error_correction_level, ErrorCorrectionLevel::Q);
        assert!(long.version >= 10, "300 bytes can't fit a small version, got {}", long.version);
    }

    #[test]
    fn stats_resolve_auto_to_the_mode_used() {
        let result = stats("HELLO", EncodingMode::Auto, MaskSetting::Auto);
        assert_eq!(result.encoding_modes, [SegmentMode::Alphanumeric]);
    }

    #[test]
    fn stats_list_every_segment_in_order() {
        let result = stats("a1234567", EncodingMode::Segmented, MaskSetting::Auto);
        assert_eq!(result.encoding_modes, [SegmentMode::Byte, SegmentMode::Numeric]);
    }

    #[test]
    fn stats_locate_a_single_segment() {
        // "HELLO" at version 1: 4 + 9 header bits, then 11 + 11 + 6 data bits.
        let result = stats("HELLO", EncodingMode::Auto, MaskSetting::Auto);
        assert_eq!(result.segments, [
            SegmentStats { mode: SegmentMode::Alphanumeric, start_bit: 0, end_bit: 41, header_bits: 13 },
        ]);
    }

    #[test]
    fn stats_locate_every_segment_of_a_segmented_code() {
        // Byte "a" is 12 + 8 bits, then Numeric "1234567" is 14 + 24 bits.
        let result = stats("a1234567", EncodingMode::Segmented, MaskSetting::Auto);
        assert_eq!(result.segments, [
            SegmentStats { mode: SegmentMode::Byte, start_bit: 0, end_bit: 20, header_bits: 12 },
            SegmentStats { mode: SegmentMode::Numeric, start_bit: 20, end_bit: 58, header_bits: 14 },
        ]);
    }

    #[test]
    fn segment_header_length_widens_with_the_version() {
        // Byte's count indicator is 16 bits from version 10.
        let data = QrData::new("ab", ErrorCorrectionLevel::L, EncodingMode::Segmented, v(10), MaskSetting::Auto);
        let segments = create_qr_code_bits(&data).unwrap().segments;
        assert_eq!(segments, [
            SegmentStats { mode: SegmentMode::Byte, start_bit: 0, end_bit: 36, header_bits: 20 },
        ]);
    }

    #[test]
    fn segments_cover_the_encoded_bits_end_to_end() {
        for text in ["01234567", "a1234567", "AB1234567", "é1234567", "HELLO WORLD 123"] {
            let data = QrData::new(text, ErrorCorrectionLevel::M, EncodingMode::Segmented, QrVersion::Auto, MaskSetting::Auto);
            let encoded = create_qr_code_bits(&data).unwrap();

            let mut position = 0;
            for segment in &encoded.segments {
                assert_eq!(segment.start_bit, position, "gap before a segment of {text:?}");
                assert!(segment.header_bits < segment.end_bit - segment.start_bit, "{text:?} has a segment with no data");
                position = segment.end_bit;
            }
            assert_eq!(position, encoded.bits.len(), "segments of {text:?} don't reach the end");
        }
    }

    #[test]
    fn segment_offsets_point_at_each_segments_mode_indicator() {
        let data = QrData::new("a1234567", ErrorCorrectionLevel::L, EncodingMode::Segmented, QrVersion::Auto, MaskSetting::Auto);
        let encoded = create_qr_code_bits(&data).unwrap();
        let indicators: Vec<&BitSlice<u8, Lsb0>> = encoded.segments.iter()
            .map(|segment| &encoded.bits[segment.start_bit..segment.start_bit + 4])
            .collect();
        assert_eq!(indicators[0], fields(&[(0b0100, 4)]).as_bitslice());
        assert_eq!(indicators[1], fields(&[(0b0001, 4)]).as_bitslice());
    }

    #[test]
    fn empty_segmented_text_has_no_segments() {
        assert!(stats("", EncodingMode::Segmented, MaskSetting::Auto).segments.is_empty());
    }

    #[test]
    fn stats_report_the_requested_mask() {
        assert_eq!(stats("HELLO", EncodingMode::Auto, MaskSetting::Exact(3)).mask, 3);
    }

    #[test]
    fn stats_report_the_mask_auto_applied() {
        let code = create_qr_code(QrData::new("HELLO", ErrorCorrectionLevel::Q, EncodingMode::Auto, QrVersion::Auto, MaskSetting::Auto)).unwrap();
        assert_eq!(Some(code.stats.mask), code.matrix.mask());
        assert!(code.stats.mask < 8);
    }

    #[test]
    fn create_qr_code_round_trips_to_the_original_string() {
        use crate::qr_code::decoder::decode_modules;

        let cases = [
            ("01234567", ErrorCorrectionLevel::M),
            ("HELLO WORLD", ErrorCorrectionLevel::Q),
            ("AC-42", ErrorCorrectionLevel::M),
            ("a1234567", ErrorCorrectionLevel::L),
            ("The quick brown fox jumps over the lazy dog.", ErrorCorrectionLevel::L),
            #[cfg(feature = "kanji")]
            ("点点abc", ErrorCorrectionLevel::H),
            ("é1234567", ErrorCorrectionLevel::M),
            ("", ErrorCorrectionLevel::M),
            ("line one\nline two", ErrorCorrectionLevel::M),
        ];
        for (original_string, level) in cases {
            let matrix = create_qr_code(QrData { text: original_string.parse().unwrap(), error_correction_level: level, encoding_mode: EncodingMode::Auto, version: QrVersion::Auto, mask: MaskSetting::Auto }).unwrap().matrix;
            let decoded_string = decode_modules(matrix.modules())
                .unwrap_or_else(|e| panic!("decode failed for {original_string:?} at {level:?}: {e:?}"));

            assert_eq!(decoded_string, original_string, "round trip mismatch for {original_string:?} at {level:?}");
        }
    }
}