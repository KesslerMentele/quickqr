use std::fmt;
use crate::qr_code::EncodingMode;

/// Errors that occur during data encoding.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum EncodeDataError {
    /// A character isn't part of the character set the mode being encoded requires.
    InvalidChar(char),
    /// A double-byte Shift-JIS code point outside the two ranges QR Kanji mode can represent.
    KanjiOutOfRange(u16),
    /// An ECI designator outside the 0..=999999 range the three encoding lengths can hold.
    EciOutOfRange(u32),
    /// The value in bits does not fit in the provided amount of bits.
    BitsWouldOverflow(u32, usize),
    /// The codeword count of the data does not match the expected value
    InvalidByteCount(usize, usize),
    /// The provided bit length is invalid.
    InvalidBitLength(usize),
    /// A structured-append sequence number or total-symbol count outside 0..=15.
    StructuredAppendOutOfRange,
    /// The requested encoding mode has not been implemented.
    NotImplemented
}
impl std::error::Error for EncodeDataError {}
impl fmt::Display for EncodeDataError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            EncodeDataError::InvalidChar(char) => {
                write!(f, "Invalid character '{}'", char)
            }
            EncodeDataError::KanjiOutOfRange(val) => {
                write!(f, "Kanji out of range: {}", val)
            }
            EncodeDataError::EciOutOfRange(val) => {
                write!(f, "Eci out of range: {}", val)
            }
            EncodeDataError::StructuredAppendOutOfRange => {
                write!(f, "Structured append out of range")
            }
            EncodeDataError::InvalidBitLength(val) => {
                write!(f, "Invalid bits length: {}", val)
            }
            EncodeDataError::InvalidByteCount(val, len) => {
                write!(f, "Invalid byte count: {} (expected {})", val, len)
            }
            EncodeDataError::BitsWouldOverflow(val, size) => {
                write!(f, "Value larger than bits length: {} does not fit in {}", val, size)
            }
            EncodeDataError::NotImplemented => {
                write!(f, "Not implemented")
            }
        }
    }
}

/// Errors to do with the [`QrVersion`](qr_code::QrVersion).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum VersionError {
    /// The provided version is out of bounds (1-40).
    OutOfBounds(u8)
}
impl std::error::Error for VersionError {}
impl fmt::Display for VersionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self { VersionError::OutOfBounds(version) => {write!(f, "QR version out of bounds: {}", version)} }
    }
}


/// Errors to do with mask application.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum ApplyMaskError {
    /// `pattern` is outside the valid 0-7 range of mask patterns.
    InvalidMaskVersion(u8),
}
impl std::error::Error for ApplyMaskError {}
impl fmt::Display for ApplyMaskError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self { ApplyMaskError::InvalidMaskVersion(version) => {
            write!(f, "invalid mask version {}", version)
        } }
    }
}

/// Errors to do with segmenting the data into codewords.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Copy)]
pub enum CreateSegmentError {
    /// The data needs more bits than the requested version holds at this error correction level.
    ExceedsVersionCapacity { needed_bits: usize, capacity_bits: usize },
    /// The data needs more bits than even version 40 holds at this error correction level.
    TooLong { needed_bits: usize },
}
impl std::error::Error for CreateSegmentError {}
impl fmt::Display for CreateSegmentError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CreateSegmentError::ExceedsVersionCapacity { .. } => {
                write!(f, "too long for the requested version and error correction level")
            }
            CreateSegmentError::TooLong { .. } => {
                write!(f, "too long for any version at this error correction level")
            }
        }
    }
}

#[cfg(test)]
/// Errors to do with decoding a QR code back into the original message
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub(crate) enum DecodeDataError {
    /// This many modules isn't a square of any QR version's width.
    InvalidModuleCount(usize),
    /// Neither copy of the format information is close enough to a valid codeword to trust, so
    /// the error correction level and mask are unknown.
    UnreadableFormatInformation,
    /// A block has more errors than its error correction codewords can repair.
    UncorrectableBlock,
    /// Ran out of bits midsegment.
    UnexpectedEnd,
    /// A mode indicator this decoder doesn't handle (ECI, FNC1, Structured Append, or a value the spec doesn't define)
    UnsupportedMode(u8),
    /// A Byte mode segment's bytes aren't valid UTF-8.
    InvalidUtf8,
    /// An Alphanumeric value outside 0..45, the width of QR's alphanumeric character set.
    InvalidAlphanumericValue(u8),
    /// A Kanji value that doesn't correspond to a Shift-JIS code point QR can represent.
    #[cfg(feature = "kanji")]
    InvalidKanjiCodePoint(u16),
}
#[cfg(test)]
impl fmt::Display for DecodeDataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecodeDataError::InvalidModuleCount(count) => {
                write!(f, "invalid module count: {}", count)
            }
            DecodeDataError::UnreadableFormatInformation => {
                write!(f, "unreadable format information")
            }
            DecodeDataError::UncorrectableBlock => {
                write!(f, "uncorrectable block")
            }
            DecodeDataError::UnexpectedEnd => {
                write!(f, "unexpected end")
            }
            DecodeDataError::UnsupportedMode(mode) => {
                write!(f, "unsupported mode: {}", mode)
            }
            DecodeDataError::InvalidUtf8 => {
                write!(f, "invalid UTF-8")
            }
            DecodeDataError::InvalidAlphanumericValue(val) => {
                write!(f, "invalid alphanumeric value: {}", val)
            }
            #[cfg(feature = "kanji")]
            DecodeDataError::InvalidKanjiCodePoint(val) => {
                write!(f, "invalid kanji code point: {}", val)
            }
        }
    }
}

/// The possible errors encountered during QR code generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CreateQrError {
    InvalidQrVersion(VersionError),
    NotSegmentable(CreateSegmentError),
    NotImplemented(EncodingMode),
    EncodingFailed(EncodeDataError),
    MaskingFailed(ApplyMaskError),
    FeatureNotEnabled(&'static str)
}
impl std::error::Error for CreateQrError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> { match self {
        CreateQrError::InvalidQrVersion(error) => Some(error),
        CreateQrError::NotSegmentable(error) => Some(error),
        CreateQrError::NotImplemented(_) => None,
        CreateQrError::EncodingFailed(error) => Some(error),
        CreateQrError::MaskingFailed(error) => Some(error),
        CreateQrError::FeatureNotEnabled(_) => None
    } }
}
impl From<CreateSegmentError> for CreateQrError {
    fn from(value: CreateSegmentError) -> Self { CreateQrError::NotSegmentable(value) }
}
impl From<VersionError> for CreateQrError {
    fn from(value: VersionError) -> Self { CreateQrError::InvalidQrVersion(value) }
}
impl From<EncodeDataError> for CreateQrError {
    fn from(value: EncodeDataError) -> Self { CreateQrError::EncodingFailed(value) }
}
impl fmt::Display for CreateQrError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CreateQrError::InvalidQrVersion(_) => {
                write!(f, "invalid QR version")
            }
            CreateQrError::NotSegmentable(_) => {
                write!(f, "qr data is too large to be segmented")
            }
            CreateQrError::NotImplemented(mode) => {
                write!(f, "requested encoding mode is not implemented: {}", mode)
            }
            CreateQrError::EncodingFailed(_) => {
                write!(f, "qr encoding failed")
            }
            CreateQrError::MaskingFailed(_) => {
                write!(f, "qr masking failed")
            }
            CreateQrError::FeatureNotEnabled(feature_name) => {
                write!(f, "feature not enabled: {}", feature_name)
            }
        }
    }
}

