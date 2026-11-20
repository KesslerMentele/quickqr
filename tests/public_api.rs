//! Uses quickqr the way a downstream crate would

use quickqr::{
    EncodingMode, ErrorCorrectionLevel, MaskSetting, QrCode, CreateQrError, QrConfiguration, QrVersionSetting,
    CreateSegmentError, SegmentMode,
};

#[test]
fn builds_with_defaults() {
    let code = QrConfiguration::new("HELLO WORLD").build().unwrap();
    assert_eq!(code.size(), 21);
    assert_eq!(code.stats().version(), 1);
    assert_eq!(code.stats().error_correction_level(), ErrorCorrectionLevel::M);
    assert_eq!(code.stats().encoding_modes(), [SegmentMode::Alphanumeric]);
}

#[test]
fn build_simply() {
    let code = QrCode::new("HELLO WORLD").unwrap();
    assert_eq!(code.size(), 21);
    assert_eq!(code.stats().version(), 1);
    assert_eq!(code.stats().error_correction_level(), ErrorCorrectionLevel::M);
    assert_eq!(code.stats().encoding_modes(), [SegmentMode::Alphanumeric]);
}

#[test]
fn settings_are_applied() {
    let code = QrConfiguration::new(String::from("HELLO"))
        .error_correction_level(ErrorCorrectionLevel::H)
        .encoding_mode(EncodingMode::Byte)
        .version(QrVersionSetting::try_from(5).unwrap())
        .mask(MaskSetting::Exact(3))
        .build()
        .unwrap();
    let stats = code.stats();
    assert_eq!(code.size(), 37);
    assert_eq!((stats.version(), stats.mask()), (5, 3));
    assert_eq!(stats.error_correction_level(), ErrorCorrectionLevel::H);
    assert_eq!(stats.encoding_modes(), [SegmentMode::Byte]);
}

#[test]
fn data_can_be_reused_for_several_codes() {
    let data = QrConfiguration::new("HELLO");
    let low = data.clone().error_correction_level(ErrorCorrectionLevel::L).build().unwrap();
    let auto = data.build().unwrap();
    assert_eq!(auto.stats().error_correction_level(), ErrorCorrectionLevel::M);
    assert_eq!(low.stats().error_correction_level(), ErrorCorrectionLevel::L);
}

#[test]
fn modules_can_be_read() {
    let code = QrConfiguration::new("HELLO").build().unwrap();
    // Each corner except bottom-right holds a finder pattern, whose outer ring is dark and whose
    // second ring is light.
    let last = code.size() - 1;
    for (row, col) in [(0, 0), (0, last), (last, 0)] {
        assert!(code.is_dark(row, col), "finder corner ({row}, {col}) should be dark");
    }
    assert!(!code.is_dark(1, 1));
}

#[test]
#[should_panic(expected = "outside a 21x21 symbol")]
fn reading_outside_the_symbol_panics() {
    // Without the bounds check, (0, 21) would silently read (1, 0).
    QrConfiguration::new("HELLO").build().unwrap().is_dark(0, 21);
}

#[test]
fn segment_stats_are_readable() {
    let code = QrConfiguration::new("a1234567").encoding_mode(EncodingMode::Segmented).build().unwrap();
    let segments = code.stats().segments();
    assert_eq!(segments.len(), 2);
    assert_eq!(segments[0].mode(), SegmentMode::Byte);
    assert_eq!(segments[0].start_bit(), 0);
    assert_eq!(segments[1].start_bit(), segments[0].end_bit());
    assert!(segments[1].header_bits() > 0);
}

#[test]
fn errors_can_be_matched_on() {
    let error = QrConfiguration::new("a".repeat(20))
        .error_correction_level(ErrorCorrectionLevel::L)
        .encoding_mode(EncodingMode::Segmented)
        .version(QrVersionSetting::try_from(1).unwrap())
        .build()
        .unwrap_err();
    assert!(matches!(error, CreateQrError::NotSegmentable(CreateSegmentError::ExceedsVersionCapacity { .. })), "{error:?}");
}

#[test]
fn public_types_are_send_and_sync() {
    fn assert<T: Send + Sync>() {}
    assert::<QrCode>();
    assert::<QrConfiguration>();
    assert::<CreateQrError>();
}
