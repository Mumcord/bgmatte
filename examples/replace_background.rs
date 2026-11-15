//! Blurs or replaces the background of a photo.
//!
//! ```text
//! cargo run --release --example replace_background -- <photo> <output.png> [blur[=0..100] | <background image>]
//! ```
//!
//! Needs ONNX Runtime: set `ORT_DYLIB_PATH` to the library (see the README). The photo is
//! cropped to a multiple of 8 pixels on each side; the result is written as PNG.

use std::sync::Arc;
use std::time::Duration;

use bgmatte::{Background, BackgroundImage, Frame, ModelShape, OnnxSegmenter, Pipeline};

fn usage() -> String {
    "usage: replace_background <photo> <output.png> [blur[=0..100] | <background image>]".into()
}

fn background(arg: Option<&str>) -> Result<Background, Box<dyn std::error::Error>> {
    Ok(match arg {
        None | Some("blur") => Background::Blur { strength: 60 },
        Some(arg) => match arg.strip_prefix("blur=") {
            Some(strength) => Background::Blur {
                strength: strength.parse()?,
            },
            None => Background::Image(Arc::new(BackgroundImage::open(arg)?)),
        },
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(input), Some(output)) = (args.first(), args.get(1)) else {
        return Err(usage().into());
    };
    let background = background(args.get(2).map(String::as_str))?;

    let mut photo = image::open(input)?.into_rgba8();
    // The pipeline needs sides divisible by 8.
    let (width, height) = (photo.width() & !7, photo.height() & !7);
    if !Pipeline::supports(width, height) {
        return Err(format!("{input} is too small ({width}x{height} after cropping)").into());
    }
    let mut photo = image::imageops::crop(&mut photo, 0, 0, width, height).to_image();

    let mut segmenter = OnnxSegmenter::new(ModelShape::Square)?;
    let mut pipeline = Pipeline::new(width, height, background)?;
    pipeline.process(
        Frame::rgba(&mut photo, width, height),
        Duration::ZERO,
        &mut segmenter,
    )?;
    photo.save(output)?;
    println!("wrote {output} ({width}x{height})");
    Ok(())
}
