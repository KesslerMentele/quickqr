//! The public-facing surface of `qr_code`: everything a caller needs to build and render a
//! QR code, without reaching into the encoding internals (segmenting, bit packing, error
//! correction, masking) that the rest of the module handles.

pub use super::{
    EncodingMode, ErrorCorrectionLevel, MaskSetting,
    QrCode, QrCreationError, QrData, QrStats, QrVersion, SegmentStats, VersionError,
    // Error and mode types the public ones above refer to.
    encoder::EncodeError,
    segmenter::{SegmentError, SegmentMode},
    structure::MaskError,
};
