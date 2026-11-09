//! The public-facing surface of `qr_code`: everything a caller needs to build and render a
//! QR code, without reaching into the encoding internals (segmenting, bit packing, error
//! correction, masking) that the rest of the module handles.

pub use super::{
    create_qr_code, EncodedData, EncodingMode,
    ErrorCorrectionLevel,
    QrCreationError, QrVersion, QrVersionError, QrData
};
