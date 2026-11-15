//! The complete effect (`Effects`): models created on demand, quality levels, background
//! changes and the "unavailable" fallback, with a known mask instead of the real model.

mod common;

use std::io::Cursor;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bgmatte::{
    Background, BackgroundImage, Config, Effects, Error, Frame, Level, ModelShape, Quality,
    Segmenter,
};
use common::{H, RectSegmenter, W, frame};

const FRAME: Duration = Duration::from_millis(33);

/// A factory that hands out rectangle masks and remembers which shapes were asked for.
fn rect_effects(config: Config) -> (Effects, Arc<Mutex<Vec<ModelShape>>>) {
    let asked = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&asked);
    let effects = Effects::with_segmenters(config, move |shape| {
        log.lock().expect("lock").push(shape);
        Ok(Box::new(RectSegmenter(shape)) as Box<dyn Segmenter>)
    });
    (effects, asked)
}

fn process(effects: &mut Effects, nv12: &mut [u8], n: u32) -> Result<(), Error> {
    effects.process(Frame::nv12(nv12, W as u32, H as u32), FRAME * n)
}

/// Mean absolute luma change over the left-most bars, far from the person.
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
fn applies_the_effect_and_reports_stats() {
    let config = Config {
        quality: Quality::Best,
        ..Config::new(30, Background::Blur { strength: 60 })
    };
    let (mut effects, asked) = rect_effects(config);
    let original = frame();
    let mut out = original.clone();
    for n in 0..5 {
        out.copy_from_slice(&original);
        process(&mut effects, &mut out, n).expect("process");
    }
    assert!(background_change(&original, &out) > 40.0);
    assert_eq!(
        out[500 * W + 640],
        original[500 * W + 640],
        "person unchanged"
    );
    let stats = effects.stats();
    assert_eq!(stats.level, Some(Level::Full));
    assert_eq!(stats.frames, 5);
    assert_eq!(stats.unavailable, None);
    assert_eq!(*asked.lock().expect("lock"), [ModelShape::Square]);
}

#[test]
fn background_none_passes_frames_unchanged() {
    let (mut effects, asked) = rect_effects(Config::new(30, Background::None));
    let original = frame();
    let mut out = original.clone();
    process(&mut effects, &mut out, 0).expect("process");
    assert_eq!(out, original);
    assert_eq!(effects.stats().level, None);
    assert!(asked.lock().expect("lock").is_empty(), "no model needed");
}

#[test]
fn switches_background_quality_and_frame_size_while_running() {
    let (mut effects, asked) = rect_effects(Config::new(30, Background::Blur { strength: 60 }));
    let original = frame();
    let mut out = original.clone();
    process(&mut effects, &mut out, 0).expect("blur");

    let image = image::RgbImage::from_pixel(64, 36, image::Rgb([0, 255, 0]));
    let mut png = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
        .expect("encode");
    let image = BackgroundImage::decode(&png).expect("decode");
    effects.set_background(Background::Image(Arc::new(image)));
    out.copy_from_slice(&original);
    process(&mut effects, &mut out, 1).expect("image");
    assert_eq!(out[400 * W + 100], 145, "green background luma");

    effects.set_quality(Quality::Light);
    out.copy_from_slice(&original);
    process(&mut effects, &mut out, 2).expect("light");
    assert_eq!(effects.stats().level, Some(Level::Light));
    assert_eq!(
        *asked.lock().expect("lock"),
        [ModelShape::Square, ModelShape::Landscape]
    );

    let mut small = vec![128u8; 640 * 360 * 3 / 2];
    effects
        .process(Frame::nv12(&mut small, 640, 360), FRAME * 3)
        .expect("new frame size");
    let mut odd = vec![0u8; 100 * 100 * 3 / 2];
    assert!(matches!(
        effects.process(Frame::nv12(&mut odd, 100, 100), FRAME * 4),
        Err(Error::UnsupportedSize { .. })
    ));
}

/// A segmenter slower than the 20 ms budget at 30 fps.
struct Slow(RectSegmenter);

impl Segmenter for Slow {
    fn shape(&self) -> ModelShape {
        self.0.shape()
    }

    fn segment(&mut self, rgb: &[f32], timestamp: Duration, mask: &mut [f32]) -> Result<(), Error> {
        std::thread::sleep(Duration::from_millis(25));
        self.0.segment(rgb, timestamp, mask)
    }
}

#[test]
fn auto_steps_down_to_light_when_the_full_level_is_too_slow() {
    let config = Config::new(30, Background::Blur { strength: 60 });
    let mut effects = Effects::with_segmenters(config, |shape| {
        Ok(Box::new(Slow(RectSegmenter(shape))) as Box<dyn Segmenter>)
    });
    let mut out = frame();
    for n in 0..20 {
        process(&mut effects, &mut out, n).expect("process");
    }
    assert_eq!(effects.stats().level, Some(Level::Light));
}

#[test]
fn an_unavailable_runtime_turns_the_effect_off_with_the_reason() {
    let config = Config::new(30, Background::Blur { strength: 60 });
    let mut effects = Effects::with_segmenters(config, |_| {
        Err(Error::Unavailable("no runtime in this test".to_owned()))
    });
    let original = frame();
    let mut out = original.clone();
    assert!(matches!(
        process(&mut effects, &mut out, 0),
        Err(Error::Unavailable(_))
    ));
    process(&mut effects, &mut out, 1).expect("passes unchanged");
    assert_eq!(out, original);
    let stats = effects.stats();
    assert_eq!(stats.level, None);
    assert_eq!(
        stats.unavailable.as_deref(),
        Some("no runtime in this test")
    );
}
