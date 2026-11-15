//! Compositing correctness on synthetic NV12 frames with a known person mask.

mod common;

use std::io::Cursor;
use std::sync::Arc;
use std::time::Duration;

use bgmatte::{Background, BackgroundImage, Error, Frame, Level, ModelShape, Pipeline, Segmenter};
use common::{
    H, PERSON_RGB, PERSON_YUV, RectSegmenter, W, edge_distance, frame, is_person, rgba_frame,
};

/// Far enough from the outline that refinement and feathering don't reach.
const MARGIN: usize = 32;

fn shape(level: Level) -> ModelShape {
    match level {
        Level::Full => ModelShape::Square,
        Level::Light => ModelShape::Landscape,
    }
}

fn run(background: Background, level: Level, frames: usize) -> (Vec<u8>, Vec<u8>) {
    let original = frame();
    let mut pipeline = Pipeline::new(W as u32, H as u32, background).expect("pipeline");
    pipeline.set_level(level);
    let mut segmenter = RectSegmenter(shape(level));
    let mut out = original.clone();
    for i in 0..frames {
        out.copy_from_slice(&original);
        pipeline
            .process(
                Frame::nv12(&mut out, W as u32, H as u32),
                Duration::from_millis(33 * i as u64),
                &mut segmenter as &mut dyn Segmenter,
            )
            .expect("process");
    }
    (original, out)
}

fn run_rgba(background: Background, level: Level) -> (Vec<u8>, Vec<u8>) {
    let original = rgba_frame();
    let mut pipeline = Pipeline::new(W as u32, H as u32, background).expect("pipeline");
    pipeline.set_level(level);
    let mut out = original.clone();
    pipeline
        .process(
            Frame::rgba(&mut out, W as u32, H as u32),
            Duration::ZERO,
            &mut RectSegmenter(shape(level)),
        )
        .expect("process");
    (original, out)
}

fn luma(nv12: &[u8], x: usize, y: usize) -> u8 {
    nv12[y * W + x]
}

fn chroma(nv12: &[u8], x: usize, y: usize) -> (u8, u8) {
    let i = W * H + (y / 2) * W + (x / 2) * 2;
    (nv12[i], nv12[i + 1])
}

/// Sample points on a grid, split into person interior and background far from the outline.
type Points = Vec<(usize, usize)>;

fn samples() -> (Points, Points) {
    let (mut person, mut background) = (Vec::new(), Vec::new());
    for y in (2..H - 2).step_by(7) {
        for x in (2..W - 2).step_by(5) {
            if edge_distance(x, y) < MARGIN {
                continue;
            }
            if is_person(x, y) {
                person.push((x, y));
            } else {
                background.push((x, y));
            }
        }
    }
    (person, background)
}

fn assert_person_unchanged(original: &[u8], out: &[u8]) {
    let (person, _) = samples();
    assert!(!person.is_empty());
    for (x, y) in person {
        assert_eq!(
            luma(out, x, y),
            luma(original, x, y),
            "person luma at {x},{y}"
        );
        assert_eq!(
            chroma(out, x, y),
            chroma(original, x, y),
            "person chroma at {x},{y}"
        );
    }
}

/// Mean absolute luma change over the background samples.
fn background_change(original: &[u8], out: &[u8]) -> f64 {
    let (_, background) = samples();
    let total: u64 = background
        .iter()
        .map(|&(x, y)| u64::from(luma(out, x, y).abs_diff(luma(original, x, y))))
        .sum();
    total as f64 / background.len() as f64
}

#[test]
fn background_none_leaves_the_frame_untouched() {
    let (original, out) = run(Background::None, Level::Full, 2);
    assert_eq!(original, out);
}

#[test]
fn blur_changes_the_background_and_keeps_the_person() {
    for level in [Level::Full, Level::Light] {
        let (original, out) = run(Background::Blur { strength: 50 }, level, 3);
        assert_person_unchanged(&original, &out);
        // Bars of 40 and 220 blur towards their mean, 130: a large change everywhere.
        let change = background_change(&original, &out);
        assert!(change > 40.0, "{level:?}: background changed by {change}");
    }
}

#[test]
fn stronger_blur_flattens_more() {
    let (original, light) = run(Background::Blur { strength: 0 }, Level::Full, 1);
    let (_, strong) = run(Background::Blur { strength: 100 }, Level::Full, 1);
    let flat = |nv12: &[u8]| {
        let (_, bg) = samples();
        bg.iter()
            .map(|&(x, y)| f64::from(luma(nv12, x, y)) - 130.0)
            .map(f64::abs)
            .sum::<f64>()
    };
    assert!(flat(&strong) < flat(&light));
    assert!(background_change(&original, &light) > 10.0);
}

fn image_background(rgb: [u8; 3]) -> Background {
    let image = image::RgbImage::from_pixel(320, 200, image::Rgb(rgb));
    let mut bytes = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
        .expect("encode");
    Background::Image(Arc::new(BackgroundImage::decode(&bytes).expect("decode")))
}

#[test]
fn image_replaces_the_background_and_keeps_the_person() {
    // Pure green: Y 145, U 54, V 34 in BT.601 limited range.
    for level in [Level::Full, Level::Light] {
        let (original, out) = run(image_background([0, 255, 0]), level, 3);
        assert_person_unchanged(&original, &out);
        let (_, background) = samples();
        for (x, y) in background {
            assert!(
                luma(&out, x, y).abs_diff(145) <= 1,
                "{level:?} luma at {x},{y}: {}",
                luma(&out, x, y)
            );
            let (u, v) = chroma(&out, x, y);
            assert!(
                u.abs_diff(54) <= 1 && v.abs_diff(34) <= 1,
                "{level:?} chroma at {x},{y}"
            );
        }
    }
}

#[test]
fn light_wrap_tints_only_the_person_edge() {
    let (original, out) = run(image_background([0, 255, 0]), Level::Full, 1);
    // Just inside the left side of the person: some green seeps in (U drops from 100 towards 54).
    let (u_edge, _) = chroma(&out, common::PERSON_X.start + 6, 400);
    assert!(u_edge < PERSON_YUV.1, "edge chroma {u_edge}");
    // Deep inside: untouched.
    let centre = (640, 500);
    assert_eq!(
        chroma(&out, centre.0, centre.1),
        chroma(&original, centre.0, centre.1)
    );
}

#[test]
fn the_edge_is_feathered_not_hard() {
    let (_, out) = run(image_background([0, 255, 0]), Level::Full, 1);
    // Across the left outline some pixels lie strictly between person and image luma.
    let row: Vec<u8> = (common::PERSON_X.start - 8..common::PERSON_X.start + 8)
        .map(|x| luma(&out, x, 400))
        .collect();
    assert!(
        row.iter().any(|&l| l > 147 && l < 198),
        "no soft transition: {row:?}"
    );
}

fn rgba(buffer: &[u8], x: usize, y: usize) -> [u8; 4] {
    let i = (y * W + x) * 4;
    [buffer[i], buffer[i + 1], buffer[i + 2], buffer[i + 3]]
}

#[test]
fn rgba_frames_keep_the_person_exact_and_the_alpha() {
    for level in [Level::Full, Level::Light] {
        let (original, out) = run_rgba(image_background([0, 255, 0]), level);
        let (person, background) = samples();
        for (x, y) in person {
            assert_eq!(
                rgba(&out, x, y),
                rgba(&original, x, y),
                "{level:?} person at {x},{y}"
            );
        }
        for (x, y) in background {
            let [r, g, b, a] = rgba(&out, x, y);
            assert!(
                r <= 3 && g >= 250 && b <= 3,
                "{level:?} background at {x},{y}: {r} {g} {b}"
            );
            assert_eq!(a, 7);
        }
        assert_eq!(&rgba(&out, 640, 500)[..3], &PERSON_RGB);
    }
}

#[test]
fn rgb_frames_are_blurred_too() {
    let original: Vec<u8> = rgba_frame()
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|&[r, g, b, _]| [r, g, b])
        .collect();
    let mut pipeline =
        Pipeline::new(W as u32, H as u32, Background::Blur { strength: 50 }).expect("pipeline");
    let mut out = original.clone();
    pipeline
        .process(
            Frame::rgb(&mut out, W as u32, H as u32),
            Duration::ZERO,
            &mut RectSegmenter(ModelShape::Square),
        )
        .expect("process");
    let (person, background) = samples();
    let at = |buffer: &[u8], x: usize, y: usize| buffer[(y * W + x) * 3];
    for (x, y) in person {
        assert_eq!(at(&out, x, y), at(&original, x, y), "person at {x},{y}");
    }
    let change: f64 = background
        .iter()
        .map(|&(x, y)| f64::from(at(&out, x, y).abs_diff(at(&original, x, y))))
        .sum::<f64>()
        / background.len() as f64;
    assert!(change > 40.0, "background changed by {change}");
}

#[test]
fn rejects_frames_that_do_not_fit() {
    let mut pipeline =
        Pipeline::new(W as u32, H as u32, Background::Blur { strength: 50 }).expect("pipeline");
    let mut segmenter = RectSegmenter(ModelShape::Square);
    let mut short = vec![0u8; W * H];
    assert!(matches!(
        pipeline.process(
            Frame::nv12(&mut short, W as u32, H as u32),
            Duration::ZERO,
            &mut segmenter
        ),
        Err(Error::FrameSize)
    ));
    let mut other = vec![0u8; 640 * 360 * 3 / 2];
    assert!(matches!(
        pipeline.process(
            Frame::nv12(&mut other, 640, 360),
            Duration::ZERO,
            &mut segmenter
        ),
        Err(Error::FrameSize)
    ));
    assert!(matches!(
        Pipeline::new(100, 100, Background::None),
        Err(Error::UnsupportedSize {
            width: 100,
            height: 100
        })
    ));
}
