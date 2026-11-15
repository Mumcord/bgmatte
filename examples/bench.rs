//! Time per 720p frame with the real models, per level and background.
//!
//! ```text
//! cargo run --release --example bench -- [photo]
//! ```
//!
//! Needs ONNX Runtime (`ORT_DYLIB_PATH`, see the README). Prints the median and 90th
//! percentile per case and the model's share. Without a photo it uses a synthetic picture,
//! which the model may not see as a person, so the edge work can be missing; pass a 16:9 photo
//! of a person for realistic numbers.

use std::sync::Arc;
use std::time::{Duration, Instant};

use bgmatte::{
    Background, BackgroundImage, Error, Frame, Level, ModelShape, OnnxSegmenter, Pipeline,
    PixelFormat, Segmenter,
};

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;
const FRAMES: usize = 300;
const WARMUP: usize = 20;

/// Wraps a segmenter and adds up the time spent in it.
struct Timed<'a> {
    inner: &'a mut OnnxSegmenter,
    spent: Duration,
}

impl Segmenter for Timed<'_> {
    fn shape(&self) -> ModelShape {
        self.inner.shape()
    }

    fn segment(&mut self, rgb: &[f32], timestamp: Duration, mask: &mut [f32]) -> Result<(), Error> {
        let start = Instant::now();
        let result = self.inner.segment(rgb, timestamp, mask);
        self.spent += start.elapsed();
        result
    }
}

fn percentile(times: &mut [Duration], p: f64) -> f64 {
    times.sort_unstable();
    let i = ((times.len() - 1) as f64 * p).round() as usize;
    times[i].as_secs_f64() * 1000.0
}

/// The picture as RGBA: the photo scaled to 720p, or a synthetic scene of bars with a lighter
/// head-and-shoulders shape in front.
fn picture(photo: Option<&str>) -> Result<image::RgbaImage, Box<dyn std::error::Error>> {
    if let Some(path) = photo {
        return Ok(image::open(path)?
            .resize_exact(WIDTH, HEIGHT, image::imageops::FilterType::Triangle)
            .into_rgba8());
    }
    Ok(image::RgbaImage::from_fn(WIDTH, HEIGHT, |x, y| {
        let (dx, dy) = (x as f32 - 640.0, y as f32 - 300.0);
        let head = dx * dx + dy * dy < 120.0 * 120.0;
        let body = y > 400 && (x as f32 - 640.0).abs() < 260.0;
        if head || body {
            image::Rgba([205, 160, 135, 255])
        } else if (x / 80) % 2 == 0 {
            image::Rgba([60, 70, 90, 255])
        } else {
            image::Rgba([180, 190, 170, 255])
        }
    }))
}

/// The same picture as NV12 (BT.601 limited range), the format most webcams deliver.
fn to_nv12(rgba: &image::RgbaImage) -> Vec<u8> {
    let (w, h) = (WIDTH as usize, HEIGHT as usize);
    let mut nv12 = vec![0u8; PixelFormat::Nv12.frame_len(WIDTH, HEIGHT)];
    let yuv = |p: &image::Rgba<u8>| {
        let [r, g, b, _] = p.0.map(f32::from);
        (
            16.0 + (65.481 * r + 128.553 * g + 24.966 * b) / 255.0,
            128.0 + (-37.797 * r - 74.203 * g + 112.0 * b) / 255.0,
            128.0 + (112.0 * r - 93.786 * g - 18.214 * b) / 255.0,
        )
    };
    for (i, p) in rgba.pixels().enumerate() {
        nv12[i] = yuv(p).0.round() as u8;
    }
    for cy in 0..h / 2 {
        for cx in 0..w / 2 {
            let (_, u, v) = yuv(rgba.get_pixel(cx as u32 * 2, cy as u32 * 2));
            nv12[w * h + cy * w + cx * 2] = u.round() as u8;
            nv12[w * h + cy * w + cx * 2 + 1] = v.round() as u8;
        }
    }
    nv12
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    bgmatte::availability()?;
    let rgba = picture(std::env::args().nth(1).as_deref())?;
    let nv12 = to_nv12(&rgba);
    let image = Background::Image(Arc::new(BackgroundImage::from_rgb(
        image::RgbImage::from_fn(1920, 1080, |x, y| {
            image::Rgb([(x / 8) as u8, (y / 5) as u8, ((x + y) / 12) as u8])
        }),
    )));
    let blur = Background::Blur { strength: 50 };
    let mut square = OnnxSegmenter::new(ModelShape::Square)?;
    let mut landscape = OnnxSegmenter::new(ModelShape::Landscape)?;
    let cases = [
        ("Full, blur", Level::Full, blur.clone(), PixelFormat::Nv12),
        ("Full, image", Level::Full, image.clone(), PixelFormat::Nv12),
        ("Light, blur", Level::Light, blur.clone(), PixelFormat::Nv12),
        ("Light, image", Level::Light, image, PixelFormat::Nv12),
        ("Full, blur, RGBA", Level::Full, blur, PixelFormat::Rgba),
    ];
    println!("{FRAMES} frames of {WIDTH}x{HEIGHT} per case (after {WARMUP} warm-up frames)");
    for (name, level, background, format) in cases {
        let segmenter = match level {
            Level::Full => &mut square,
            Level::Light => &mut landscape,
        };
        let mut timed = Timed {
            inner: segmenter,
            spent: Duration::ZERO,
        };
        let mut pipeline = Pipeline::new(WIDTH, HEIGHT, background)?;
        pipeline.set_level(level);
        let source: &[u8] = match format {
            PixelFormat::Nv12 => &nv12,
            _ => &rgba,
        };
        let mut frame = source.to_vec();
        let mut times = Vec::with_capacity(FRAMES);
        for i in 0..WARMUP + FRAMES {
            if i == WARMUP {
                timed.spent = Duration::ZERO;
            }
            frame.copy_from_slice(source);
            // 30 fps timestamps, so Light runs its model on every second frame.
            let timestamp = Duration::from_secs_f64(i as f64 / 30.0);
            let start = Instant::now();
            pipeline.process(
                Frame::new(&mut frame, WIDTH, HEIGHT, format),
                timestamp,
                &mut timed,
            )?;
            if i >= WARMUP {
                times.push(start.elapsed());
            }
        }
        let model_ms = timed.spent.as_secs_f64() * 1000.0 / FRAMES as f64;
        let median = percentile(&mut times, 0.5);
        let p90 = percentile(&mut times, 0.9);
        println!(
            "{name:<17} median {median:6.2} ms   p90 {p90:6.2} ms   model {model_ms:5.2} ms/frame"
        );
    }
    Ok(())
}
