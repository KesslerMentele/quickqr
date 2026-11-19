use std::fmt;
use bitvec::prelude::*;
use std::ops::RangeInclusive;
use crate::bits::bytes_to_bits;
use crate::blocks::interleave;
use crate::EncodeDataError;
use crate::encoder::{apply_formatting, encode_data_by_encoding_plan, encode_single_segment};
pub(crate) use crate::errors::{CreateQrError, VersionError};
use crate::segmenter::{segment, SegmentMode, best_single_mode};
use crate::structure::QrMatrix;


/// The symbol versions QR codes come in.
const VERSIONS: RangeInclusive<u8> = 1..=40;

/// The versions that can be used to create a QR code. Either [`Auto`](QrVersion::Auto) or a value from 1-40.
/// Defaults to [QrVersion::Auto]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum QrVersion {
    #[default]
    Auto,
    Exact(u8),
}
impl fmt::Display for QrVersion {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            QrVersion::Auto => { write!(f, "auto") }
            QrVersion::Exact(version) => {write!(f, "version {}", version)}
        }
    }
}
impl TryFrom<u8> for QrVersion {
    type Error = VersionError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if VERSIONS.contains(&value) {Ok(QrVersion::Exact(value))} else {Err(VersionError::OutOfBounds(value)) }
    }
}

/// The encoding mode that can be used to create a QR code.
/// Defaults to [EncodingMode::Auto]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum EncodingMode {
    Numeric,
    Alphanumeric,
    Byte,
    Kanji,
    Eci,
    Fnc1,
    StructuredAppend,
    #[default]
    Auto,
    Segmented
}
impl TryFrom<SegmentMode> for EncodingMode {
    type Error = CreateQrError;

    fn try_from(value: SegmentMode) -> Result<Self, CreateQrError> {
        match value {
            SegmentMode::Numeric => {Ok(EncodingMode::Numeric)}
            SegmentMode::Alphanumeric => { Ok(EncodingMode::Alphanumeric)}
            SegmentMode::Byte => {Ok(EncodingMode::Byte)}

            SegmentMode::Kanji => {
                if cfg!(feature = "kanji") {
                    Ok(EncodingMode::Kanji)

                } else {
                    Err(CreateQrError::FeatureNotEnabled("kanji"))
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

/// The error correction level that can be used to create a QR code.
/// Defaults to [ErrorCorrectionLevel::M]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ErrorCorrectionLevel {
    L,
    #[default]
    M,
    Q,
    H
}
 impl fmt::Display for ErrorCorrectionLevel {
     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
         write!(f, "{:?}", self)
     }
 }

/// Encoded segment bits and the version they were sized for.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct EncodedData {
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
pub struct SegmentStats {
    pub(crate) mode: SegmentMode,
    /// The first bit of the segment's header.
    pub(crate) start_bit: usize,
    /// One past the segment's last bit, which is where the next segment starts.
    pub(crate) end_bit: usize,
    /// The mode indicator and character count: the segment's data starts at `start_bit + header_bits`.
    pub(crate) header_bits: usize,
}
impl SegmentStats {
    /// The mode this segment is encoded in.
    pub fn mode(&self) -> SegmentMode {
        self.mode
    }

    /// The bit offset of the segment's header (its mode indicator).
    pub fn start_bit(&self) -> usize {
        self.start_bit
    }

    /// One past the segment's last bit, which is where the next segment starts.
    pub fn end_bit(&self) -> usize {
        self.end_bit
    }

    /// The length of the mode indicator and character count. The segment's data starts at
    /// [`start_bit`](Self::start_bit)` + header_bits`.
    pub fn header_bits(&self) -> usize {
        self.header_bits
    }
}
impl fmt::Display for SegmentStats {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "mode:{}, start:{}, end:{}, header:{}", self.mode, self.start_bit, self.end_bit, self.header_bits)
    }
}

/// Detailed information on the settings of a created QR Code.
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
impl QrStats {
    /// The symbol version used, 1-40.
    pub fn version(&self) -> u8 {
        self.version
    }

    pub fn error_correction_level(&self) -> ErrorCorrectionLevel {
        self.error_correction_level
    }

    /// The mask pattern applied, 0-7.
    pub fn mask(&self) -> u8 {
        self.mask
    }

    /// The mode of every segment, in order.
    pub fn encoding_modes(&self) -> &[SegmentMode] {
        &self.encoding_modes
    }

    /// Where every segment sits in the encoded data, in order.
    pub fn segments(&self) -> &[SegmentStats] {
        &self.segments
    }
}
impl fmt::Display for QrStats {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "version: {}, error correction level: {}, mask: {},encoding modes: {:?}, segments: {:?}",
               self.version, self.error_correction_level, self.mask, self.encoding_modes, self.segments
        )
    }
}

/// A finished QR code symbol. Holds the [`matrix`](QrMatrix) and [`stats`](QrStats) for the code.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QrCode {
    pub(crate) matrix: QrMatrix,
    stats: QrStats,
}
impl QrCode {
    /// Generates a [QrCode] with the default settings.
    pub fn new<S: Into<String>>(data: S) -> Result<QrCode, CreateQrError> {
        QrCode::with_config(&QrConfiguration::new(data))
    }
    /// Generates a [QrCode] with the provided settings.
    pub fn with_config(data: &QrConfiguration) -> Result<QrCode, CreateQrError> {
        let encoded_data = create_qr_code_bits(data)?;
        let segments = encoded_data.segments.clone();
        let mut matrix = QrMatrix::new(encoded_data.version, data.error_correction_level);
        let error_corrected_data = create_error_corrected_data(encoded_data, data.error_correction_level)?;
        matrix.place_data(&error_corrected_data);
        matrix.set_mask(data.mask).map_err(CreateQrError::MaskingFailed)?;
        let stats = QrStats {
            version: matrix.version(),
            error_correction_level: data.error_correction_level,
            mask: matrix.mask().expect("set_mask succeeded, so a mask is set"),
            encoding_modes: segments.iter().map(|segment| segment.mode).collect(),
            segments,
        };
        Ok(QrCode { matrix, stats })
    }

    /// Width and height of the symbol, in modules (21 for version 1, then 4 more per version).
    /// Doesn't include the quiet zone, the light border scanners need around the symbol.
    pub fn size(&self) -> usize {
        self.matrix.size()
    }

    /// Whether the module at `row`, `col` is dark. `(0, 0)` is the top-left corner.
    ///
    /// # Panics
    ///
    /// If `row` or `col` is not less than [`size`](Self::size).
    pub fn is_dark(&self, row: usize, col: usize) -> bool {
        let size = self.size();
        assert!(row < size && col < size, "module ({row}, {col}) is outside a {size}x{size} symbol");
        self.matrix.is_dark(row, col)
    }

    /// The settings the code was actually built with, with every `Auto` resolved.
    pub fn stats(&self) -> &QrStats {
        &self.stats
    }
}
impl fmt::Display for QrCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "matrix: {}, stats: {}", self.matrix, self.stats)
    }
}


/// The eight mask patterns.
#[cfg(feature = "serde")]
const MASK_PATTERNS: RangeInclusive<u8> = 0..=7;

/// The mask patterns for creating a QR code.
/// Defaults to [MaskSetting::Auto]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum MaskSetting {
    #[default]
    Auto,
    Exact(u8),
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


/// The text to encode and how to encode it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QrConfiguration {
    pub(crate) text: String,
    pub(crate) error_correction_level: ErrorCorrectionLevel,
    pub(crate) encoding_mode: EncodingMode,
    pub(crate) version: QrVersion,
    pub(crate) mask: MaskSetting
}
impl QrConfiguration {
    /// Settings for encoding `text`, with every other setting at its default.
    pub fn new<S: Into<String> >(text: S ) -> QrConfiguration {
        QrConfiguration {
            text: text.into(),
            error_correction_level: ErrorCorrectionLevel::default(),
            encoding_mode: EncodingMode::default(),
            version: QrVersion::default(),
            mask: MaskSetting::default(),
        }
    }

    /// Every setting at once.
    #[cfg(test)]
    pub(crate) fn from_parts(text: &str, error_correction_level: ErrorCorrectionLevel, encoding_mode: EncodingMode, version: QrVersion, mask: MaskSetting) -> QrConfiguration {
        QrConfiguration { text: text.into(), error_correction_level, encoding_mode, version, mask }
    }

    /// How much damage the code can recover from. Higher levels hold less data.
    pub fn error_correction_level(mut self, level: ErrorCorrectionLevel) -> Self {
        self.error_correction_level = level;
        self
    }

    /// How the text is turned into bits.
    pub fn encoding_mode(mut self, mode: EncodingMode) -> Self {
        self.encoding_mode = mode;
        self
    }

    /// The symbol version, which sets the code's size.
    pub fn version(mut self, version: QrVersion) -> Self {
        self.version = version;
        self
    }

    /// The mask pattern applied over the data.
    pub fn mask(mut self, mask: MaskSetting) -> Self {
        self.mask = mask;
        self
    }

    /// Encodes the text into a [`QrCode`].
    ///
    /// # Errors
    ///
    /// If the text doesn't fit the requested version (or any version in the case of
    /// [`QrVersion::Auto`](QrVersion::Auto)).
    ///
    /// If the data contains characters the requested encoding mode can't represent.
    ///
    /// If the requested mode isn't available.
    pub fn build(&self) -> Result<QrCode, CreateQrError> {
        QrCode::with_config(self)
    }
}
impl fmt::Display for QrConfiguration {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "text:{}, error correction level: {}, encoding mode: {}, version: {}, mask: {}",
               self.text, self.error_correction_level, self.encoding_mode, self.version, self.mask
        )
    }
}

/// Serde support for settings that are either `Auto` or a number in a fixed range, as
/// [`QrVersion`] and [`MaskSetting`] are.
#[cfg(feature = "serde")]
mod auto_or_number {
    use std::{fmt, ops::RangeInclusive};
    use serde::de::{self, Unexpected, Visitor};
    use serde::{Deserializer, Serializer};
    use super::{MaskSetting, QrVersion, MASK_PATTERNS, VERSIONS};

    const AUTO: &str = "Auto";

    /// `None` is `Auto`.
    fn serialize<S: Serializer>(value: Option<u8>, serializer: S) -> Result<S::Ok, S::Error> {
        match value {
            None => serializer.serialize_str(AUTO),
            Some(number) => serializer.serialize_u8(number),
        }
    }

    /// Accepts `"Auto"`as `None`, or a number in `range` given as an integer or a
    /// numeric string. Uses `deserialize_any`, so it needs a self-describing format (JSON, TOML,
    /// YAML, etc.)
    fn deserialize<'de, D: Deserializer<'de>>(deserializer: D, what: &'static str, range: RangeInclusive<u8>) -> Result<Option<u8>, D::Error> {
        deserializer.deserialize_any(AutoOrNumber { what, range })
    }

    struct AutoOrNumber {
        what: &'static str,
        range: RangeInclusive<u8>,
    }

    impl AutoOrNumber {
        fn in_range<E: de::Error>(&self, number: i128, unexpected: Unexpected) -> Result<Option<u8>, E> {
            u8::try_from(number).ok()
                .filter(|number| self.range.contains(number))
                .map(Some)
                .ok_or_else(|| E::invalid_value(unexpected, self))
        }
    }

    impl<'de> Visitor<'de> for AutoOrNumber {
        type Value = Option<u8>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "\"{AUTO}\" or a {} from {} to {}", self.what, self.range.start(), self.range.end())
        }

        fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
            self.in_range(v.into(), Unexpected::Signed(v))
        }

        // JSON gives non-negative integers as u64; TOML gives every integer as i64.
        fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
            self.in_range(v.into(), Unexpected::Unsigned(v))
        }

        fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
            if v.eq_ignore_ascii_case(AUTO) {
                return Ok(None);
            }
            match v.trim().parse::<i128>() {
                Ok(number) => self.in_range(number, Unexpected::Str(v)),
                Err(_) => Err(E::invalid_value(Unexpected::Str(v), &self)),
            }
        }
    }

    impl serde::Serialize for QrVersion {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            serialize(match self { QrVersion::Auto => None, QrVersion::Exact(version) => Some(*version) }, serializer)
        }
    }

    impl<'de> serde::Deserialize<'de> for QrVersion {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            Ok(deserialize(deserializer, "QR version", VERSIONS)?.map_or(QrVersion::Auto, QrVersion::Exact))
        }
    }

    impl serde::Serialize for MaskSetting {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            serialize(match self { MaskSetting::Auto => None, MaskSetting::Exact(mask) => Some(*mask) }, serializer)
        }
    }

    impl<'de> serde::Deserialize<'de> for MaskSetting {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            Ok(deserialize(deserializer, "mask pattern", MASK_PATTERNS)?.map_or(MaskSetting::Auto, MaskSetting::Exact))
        }
    }
}

/// Creates the [EncodedData] with the provided [QrConfiguration].
pub(crate) fn create_qr_code_bits(
    data: &QrConfiguration
) -> Result<EncodedData, CreateQrError> {
    
    let mode = match data.encoding_mode {
        EncodingMode::Segmented => {
            let segment_plan = segment(&data.text, data.version, data.error_correction_level)?;
            return encode_data_by_encoding_plan(&data.text, segment_plan);
        }
        EncodingMode::Auto => best_single_mode(&data.text),
        EncodingMode::Numeric => SegmentMode::Numeric,
        EncodingMode::Alphanumeric => SegmentMode::Alphanumeric,
        EncodingMode::Byte => SegmentMode::Byte,
        #[cfg(feature = "kanji")]
        EncodingMode::Kanji => {
            SegmentMode::Kanji
        }
        #[cfg(not(feature = "kanji"))]
        EncodingMode::Kanji => {
            return Err(CreateQrError::FeatureNotEnabled("kanji"))
        },
        mode @ (EncodingMode::Eci | EncodingMode::Fnc1 | EncodingMode::StructuredAppend) => {
            return Err(CreateQrError::NotImplemented(mode));
        }
    };
    encode_single_segment(&data.text, mode, data.version, data.error_correction_level)
}

/// Takes [EncodedData] and interleaves it, returning a [BitVec] representing the raw bits to be
/// placed into the modules.
fn create_error_corrected_data(encoded_data: EncodedData, error_correction_level:ErrorCorrectionLevel) -> Result<BitVec<u8>, EncodeDataError> {
    let bit_vec = interleave(
        &bytes_to_bits(
            &apply_formatting(
                encoded_data.bits,
                encoded_data.version,
                error_correction_level)?
        ),
        encoded_data.version, error_correction_level);
    Ok(bit_vec)
}


#[cfg(test)]
mod tests {
    use crate::bits::write_bits;
    use crate::decoder::decode_modules;
    use crate::errors::{CreateSegmentError, EncodeDataError};
    use super::*;

    fn v(version: u8) -> QrVersion { QrVersion::Exact(version) }

    /// Builds an expected bitstream from `(value, bit_length)` fields, in order.
    fn fields(fields: &[(u32, usize)]) -> BitVec<u8, Lsb0> {
        let mut bits = BitVec::new();
        for &(value, bit_length) in fields {
            write_bits(&mut bits, value, bit_length).unwrap();
        }
        bits
    }

    #[test]
    fn segmented_single_mode_matches_the_iso_18004_example() {
        // ISO/IEC 18004 Annex I: "01234567" at 1-M is mode 0001, count 8 in 10 bits, then data.
        let expected = fields(&[(0b0001, 4), (8, 10), (12, 10), (345, 10), (67, 7)]);
        let bits = create_qr_code_bits(&QrConfiguration::from_parts("01234567", ErrorCorrectionLevel::M, EncodingMode::Segmented, v(1), MaskSetting::Auto)).unwrap().bits;
        assert_eq!(bits, expected);
    }

    #[test]
    fn segmented_writes_a_header_for_every_segment() {
        // Byte "a", then Numeric "1234567" (see segmenter's worked example).
        let expected = fields(&[
            (0b0100, 4), (1, 8), (b'a' as u32, 8),
            (0b0001, 4), (7, 10), (123, 10), (456, 10), (7, 4),
        ]);
        let bits = create_qr_code_bits(&QrConfiguration::from_parts("a1234567", ErrorCorrectionLevel::L, EncodingMode::Segmented, QrVersion::Auto, MaskSetting::Auto)).unwrap().bits;
        assert_eq!(bits, expected);
    }

    #[test]
    fn segmented_length_matches_the_segment_plan() {
        for data in ["01234567", "a1234567", "AB1234567", "é1234567", "点点abc", "HELLO WORLD 123"] {
            let plan = segment(data, QrVersion::Auto, ErrorCorrectionLevel::M).unwrap();
            let bits = create_qr_code_bits(&QrConfiguration::from_parts(data, ErrorCorrectionLevel::M, EncodingMode::Segmented, QrVersion::Auto, MaskSetting::Auto )).unwrap().bits;
            assert_eq!(bits.len(), plan.data_bits_length, "length mismatch for {data:?}");
        }
    }

    #[test]
    fn segmented_count_indicator_widens_with_the_version() {
        // Byte's count indicator is 8 bits up to version 9, 16 bits from version 10.
        let expected = fields(&[(0b0100, 4), (2, 16), (b'a' as u32, 8), (b'b' as u32, 8)]);
        let bits = create_qr_code_bits(&QrConfiguration::from_parts("ab", ErrorCorrectionLevel::L, EncodingMode::Segmented, v(10), MaskSetting::Auto)).unwrap().bits;
        assert_eq!(bits, expected);
    }

    #[test]
    fn segmented_rejects_data_that_does_not_fit_a_fixed_version() {
        let result = create_qr_code_bits(&QrConfiguration::from_parts(&"a".repeat(20), ErrorCorrectionLevel::L, EncodingMode::Segmented, v(1), MaskSetting::Auto));
        assert!(matches!(result, Err(CreateQrError::NotSegmentable(CreateSegmentError::ExceedsVersionCapacity { .. }))));
    }

    #[test]
    fn auto_detect_matches_the_iso_18004_alphanumeric_example() {
        // ISO/IEC 18004 Annex I: "AC-42" is mode 0010, count 5 in 9 bits, then data.
        let expected = fields(&[(0b0010, 4), (5, 9), (10 * 45 + 12, 11), (41 * 45 + 4, 11), (2, 6)]);
        let bits = create_qr_code_bits(&QrConfiguration::from_parts("AC-42", ErrorCorrectionLevel::M, EncodingMode::Auto, QrVersion::Auto, MaskSetting::Auto)).unwrap().bits;
        assert_eq!(bits, expected);
    }

    #[test]
    #[cfg(feature = "kanji")]
    fn kanji_count_indicator_counts_chars_not_bytes() {
        // 点 is 3 UTF-8 bytes, but the count is 1 char. Data is 0x935F reduced to 0x0D9F.
        let expected = fields(&[(0b1000, 4), (1, 8), (0x0D9F, 13)]);
        let bits = create_qr_code_bits(&QrConfiguration::from_parts("点", ErrorCorrectionLevel::M, EncodingMode::Kanji, QrVersion::Auto, MaskSetting::Auto)).unwrap().bits;
        assert_eq!(bits, expected);
    }

    #[test]
    fn byte_count_indicator_counts_utf8_bytes() {
        // é is 2 UTF-8 bytes: 0xC3 0xA9.
        let expected = fields(&[(0b0100, 4), (2, 8), (0xC3, 8), (0xA9, 8)]);
        let bits = create_qr_code_bits(&QrConfiguration::from_parts("é", ErrorCorrectionLevel::M, EncodingMode::Byte, QrVersion::Auto, MaskSetting::Auto)).unwrap().bits;
        assert_eq!(bits, expected);
    }

    #[test]
    fn fixed_mode_rejects_data_that_does_not_fit_a_fixed_version() {
        let result = create_qr_code_bits(&QrConfiguration::from_parts(&"a".repeat(20), ErrorCorrectionLevel::L, EncodingMode::Byte, v(1), MaskSetting::Auto));
        assert!(result.is_err(), "20 bytes needs 172 bits but 1-L only holds 152");
    }

    #[test]
    fn auto_detect_widens_the_count_in_a_larger_version_group() {
        // 300 bytes overflows every Small version at L, so the count indicator is 16 bits.
        let bits = create_qr_code_bits(&QrConfiguration::from_parts(&"a".repeat(300), ErrorCorrectionLevel::L, EncodingMode::Auto, QrVersion::Auto, MaskSetting::Auto)).unwrap().bits;
        assert_eq!(bits.len(), 4 + 16 + 300 * 8);
        assert_eq!(bits[..20], fields(&[(0b0100, 4), (300, 16)])[..]);
    }

    #[test]
    fn auto_detect_rejects_data_too_long_for_version_40() {
        let result = create_qr_code_bits(&QrConfiguration::from_parts(&"a".repeat(3000), ErrorCorrectionLevel::L, EncodingMode::Auto, QrVersion::Auto, MaskSetting::Auto));
        assert!(matches!(result, Err(CreateQrError::NotSegmentable(CreateSegmentError::TooLong { needed_bits: 24020 }))));
    }

    #[test]
    fn fixed_mode_rejects_chars_outside_its_set() {
        let result = create_qr_code_bits(&QrConfiguration::from_parts("12a", ErrorCorrectionLevel::M, EncodingMode::Numeric, QrVersion::Auto, MaskSetting::Auto));
        assert!(matches!(result, Err(CreateQrError::EncodingFailed(EncodeDataError::InvalidChar('a')))));
    }

    #[test]
    fn prefix_modes_are_not_implemented() {
        for mode in [EncodingMode::Eci, EncodingMode::Fnc1, EncodingMode::StructuredAppend] {
            let result = create_qr_code_bits(&QrConfiguration::from_parts("1", ErrorCorrectionLevel::M, mode, QrVersion::Auto, MaskSetting::Auto));
            assert!(matches!(result, Err(CreateQrError::NotImplemented(_))));
        }
    }

    #[test]
    fn segmented_empty_input_is_empty() {
        let bits = create_qr_code_bits(&QrConfiguration::from_parts("", ErrorCorrectionLevel::M, EncodingMode::Segmented, QrVersion::Auto, MaskSetting::Auto)).unwrap().bits;
        assert!(bits.is_empty());
    }

    fn stats(text: &str, encoding_mode: EncodingMode, mask: MaskSetting) -> QrStats {
        QrCode::with_config(&QrConfiguration::from_parts(text, ErrorCorrectionLevel::Q, encoding_mode, QrVersion::Auto, mask)).unwrap().stats
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
        let data = QrConfiguration::from_parts("ab", ErrorCorrectionLevel::L, EncodingMode::Segmented, v(10), MaskSetting::Auto);
        let segments = create_qr_code_bits(&data).unwrap().segments;
        assert_eq!(segments, [
            SegmentStats { mode: SegmentMode::Byte, start_bit: 0, end_bit: 36, header_bits: 20 },
        ]);
    }

    #[test]
    fn segments_cover_the_encoded_bits_end_to_end() {
        for text in ["01234567", "a1234567", "AB1234567", "é1234567", "HELLO WORLD 123"] {
            let data = QrConfiguration::from_parts(text, ErrorCorrectionLevel::M, EncodingMode::Segmented, QrVersion::Auto, MaskSetting::Auto);
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
        let data = QrConfiguration::from_parts("a1234567", ErrorCorrectionLevel::L, EncodingMode::Segmented, QrVersion::Auto, MaskSetting::Auto);
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
        let code = QrCode::with_config(&QrConfiguration::from_parts("HELLO", ErrorCorrectionLevel::Q, EncodingMode::Auto, QrVersion::Auto, MaskSetting::Auto)).unwrap();
        assert_eq!(Some(code.stats.mask), code.matrix.mask());
        assert!(code.stats.mask < 8);
    }

    #[test]
    fn create_qr_code_round_trips_to_the_original_string() {

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
            let matrix = QrCode::with_config(&QrConfiguration { text: original_string.into(), error_correction_level: level, encoding_mode: EncodingMode::Auto, version: QrVersion::Auto, mask: MaskSetting::Auto }).unwrap().matrix;
            let decoded_string = decode_modules(matrix.modules())
                .unwrap_or_else(|e| panic!("decode failed for {original_string:?} at {level:?}: {e:?}"));

            assert_eq!(decoded_string, original_string, "round trip mismatch for {original_string:?} at {level:?}");
        }
    }
}
#[cfg(all(test, feature = "serde"))]
mod serde_tests {
    use super::*;
    use serde::Deserialize;
    use serde::de::{value::Error as ValueError, IntoDeserializer};

    fn round_trip<T>(value: T)
    where T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + fmt::Debug {
        let json = serde_json::to_string(&value).unwrap();
        let back: T = serde_json::from_str(&json)
            .unwrap_or_else(|e| panic!("{value:?} serialized as {json} but didn't deserialize: {e}"));
        assert_eq!(back, value, "{value:?} serialized as {json}");
    }

    #[test]
    fn version_round_trips() {
        round_trip(QrVersion::Auto);
        round_trip(QrVersion::Exact(1));
        round_trip(QrVersion::Exact(40));
    }

    #[test]
    fn mask_round_trips() {
        round_trip(MaskSetting::Auto);
        round_trip(MaskSetting::Exact(0));
        round_trip(MaskSetting::Exact(7));
    }

    #[test]
    fn qr_data_round_trips() {
        round_trip(QrConfiguration::from_parts("HELLO", ErrorCorrectionLevel::Q, EncodingMode::Auto, QrVersion::Exact(5), MaskSetting::Exact(3)));
        round_trip(QrConfiguration::from_parts("HELLO", ErrorCorrectionLevel::M, EncodingMode::Auto, QrVersion::Auto, MaskSetting::Auto));
    }

    #[test]
    fn version_accepts_a_number_or_a_numeric_string() {
        assert_eq!(serde_json::from_str::<QrVersion>("5").unwrap(), QrVersion::Exact(5));
        assert_eq!(serde_json::from_str::<QrVersion>("\"5\"").unwrap(), QrVersion::Exact(5));
        assert_eq!(serde_json::from_str::<QrVersion>("\"auto\"").unwrap(), QrVersion::Auto);
    }

    #[test]
    fn version_accepts_signed_integers() {
        // Formats like TOML hand every integer to serde as an i64.
        let deserializer: serde::de::value::I64Deserializer<ValueError> = 5i64.into_deserializer();
        assert_eq!(QrVersion::deserialize(deserializer).unwrap(), QrVersion::Exact(5));
        let deserializer: serde::de::value::I64Deserializer<ValueError> = 41i64.into_deserializer();
        assert!(QrVersion::deserialize(deserializer).is_err());
    }

    #[test]
    fn version_rejects_out_of_range_values() {
        for json in ["0", "41", "\"99\"", "-1", "\"five\""] {
            assert!(serde_json::from_str::<QrVersion>(json).is_err(), "{json} should be rejected");
        }
    }

    #[test]
    fn mask_rejects_out_of_range_values() {
        for json in ["8", "\"8\"", "255"] {
            assert!(serde_json::from_str::<MaskSetting>(json).is_err(), "{json} should be rejected");
        }
    }
}
