//! The bundled models through ONNX Runtime. Ignored by default because they need the library:
//! set `ORT_DYLIB_PATH` to ONNX Runtime 1.23 or newer and run
//! `cargo test -- --include-ignored` (CI does).

mod common;

use std::time::Duration;

use bgmatte::{
    Background, Config, Effects, Frame, Level, ModelShape, OnnxSegmenter, Pipeline, Quality,
    Segmenter,
};
use common::{H, W, frame};

fn require_runtime() {
    if let Err(err) = bgmatte::availability() {
        panic!("set ORT_DYLIB_PATH to ONNX Runtime 1.23 or newer: {err}");
    }
}

/// Mean absolute luma change over the bars left of the person.
fn background_change(original: &[u8], out: &[u8]) -> f64 {
    let points: Vec<usize> = (100..600)
        .step_by(7)
        .flat_map(|y| (8..300).step_by(5).map(move |x| y * W + x))
        .collect();
    let total: u64 = points
        .iter()
        .map(|&i| u64::from(out[i].abs_diff(original[i])))
        .sum();
    total as f64 / points.len() as f64
}

#[test]
#[ignore = "needs ONNX Runtime (ORT_DYLIB_PATH)"]
fn the_models_run_and_change_the_background() {
    require_runtime();
    for (shape, level) in [
        (ModelShape::Square, Level::Full),
        (ModelShape::Landscape, Level::Light),
    ] {
        let mut segmenter = OnnxSegmenter::new(shape).expect("model loads");
        let (mw, mh) = shape.size();
        let rgb = vec![0.5f32; 3 * mw * mh];
        let mut mask = vec![-1.0f32; mw * mh];
        segmenter
            .segment(&rgb, Duration::ZERO, &mut mask)
            .expect("segment");
        assert!(mask.iter().all(|v| (0.0..=1.0).contains(v)), "{shape:?}");

        let mut pipeline =
            Pipeline::new(W as u32, H as u32, Background::Blur { strength: 60 }).expect("pipeline");
        pipeline.set_level(level);
        let original = frame();
        let mut out = original.clone();
        pipeline
            .process(
                Frame::nv12(&mut out, W as u32, H as u32),
                Duration::ZERO,
                &mut segmenter,
            )
            .expect("process");
        let change = background_change(&original, &out);
        assert!(change > 2.0, "{shape:?}: background changed by {change}");
    }
}

#[test]
#[ignore = "needs ONNX Runtime (ORT_DYLIB_PATH)"]
fn effects_load_both_models_on_demand() {
    require_runtime();
    let config = Config {
        quality: Quality::Best,
        ..Config::new(30, Background::Blur { strength: 60 })
    };
    let mut effects = Effects::new(config);
    let mut out = frame();
    for n in 0..3 {
        effects
            .process(
                Frame::nv12(&mut out, W as u32, H as u32),
                Duration::from_millis(33 * n),
            )
            .expect("full level");
    }
    effects.set_quality(Quality::Light);
    effects
        .process(
            Frame::nv12(&mut out, W as u32, H as u32),
            Duration::from_millis(99),
        )
        .expect("light level");
    let stats = effects.stats();
    assert_eq!(stats.level, Some(Level::Light));
    assert_eq!(stats.frames, 4);
    assert_eq!(stats.unavailable, None);
}
