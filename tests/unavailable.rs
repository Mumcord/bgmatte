//! Without ONNX Runtime the effect is off with a reason and frames pass unchanged. Its own test
//! binary, because ONNX Runtime is loaded (or fails to load) once per process.

mod common;

use std::time::Duration;

use bgmatte::{Background, Config, Effects, Error, Frame, ModelShape, OnnxSegmenter};
use common::{H, W, frame};

#[test]
fn a_missing_library_turns_the_effect_off_with_a_reason() {
    let missing = std::env::temp_dir().join("bgmatte-no-such-dir/libonnxruntime.so");
    let Err(Error::Unavailable(reason)) = bgmatte::init_runtime(&missing) else {
        panic!("a missing library must be reported as unavailable");
    };
    assert!(!reason.is_empty());
    // The first attempt decides for the whole process.
    assert!(matches!(bgmatte::availability(), Err(Error::Unavailable(r)) if r == reason));
    assert!(matches!(
        OnnxSegmenter::new(ModelShape::Square),
        Err(Error::Unavailable(_))
    ));

    let mut effects = Effects::new(Config::new(30, Background::Blur { strength: 50 }));
    let original = frame();
    let mut out = original.clone();
    let result = effects.process(Frame::nv12(&mut out, W as u32, H as u32), Duration::ZERO);
    assert!(matches!(result, Err(Error::Unavailable(_))));
    effects
        .process(
            Frame::nv12(&mut out, W as u32, H as u32),
            Duration::from_millis(33),
        )
        .expect("later frames pass");
    assert_eq!(out, original);
    let stats = effects.stats();
    assert_eq!(stats.level, None);
    assert_eq!(stats.unavailable.as_deref(), Some(reason.as_str()));
}
