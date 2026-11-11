//! The public-facing surface of `qr_code`: everything a caller needs to build and render a
//! QR code, without reaching into the encoding internals (segmenting, bit packing, error
//! correction, masking) that the rest of the module handles.

mod qr_code;
mod errors;
mod segmenter;
pub mod structure;
mod penalty_score;
mod gf_math;
mod encoder;
mod decoder;
mod capacity;
mod blocks;
mod bits;


pub use {
    // Error and mode types the public ones above refer to.
    segmenter::SegmentMode,
    errors::*
};

pub use qr_code::{
    EncodingMode, ErrorCorrectionLevel, MaskSetting,
    QrCode, QrData, QrStats, QrVersion, SegmentStats,
};