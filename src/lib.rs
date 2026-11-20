//! A QR Code encoder
//!
//! This crate provides an encoder for standard QR codes.
//! Allows for numbers, text, kanji and binary encoding, while providing optimally-packed data
//! and best-fit version selection.

mod qr_code;
mod errors;
mod segmenter;
mod structure;
mod penalty_score;
mod gf_math;
mod encoder;
#[cfg(test)]
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
    QrCode, QrConfiguration, QrStats, QrVersionSetting, SegmentStats,
};