//! The per-frame path allocates nothing once the pipeline is warmed up.

mod common;

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::Arc;
use std::time::Duration;

use bgmatte::{
    Background, BackgroundImage, Frame, Level, ModelShape, Pipeline, PixelFormat, Segmenter,
};
use common::{H, RectSegmenter, W, frame, rgba_frame};

struct Counting;

thread_local! {
    static COUNT: Cell<Option<usize>> = const { Cell::new(None) };
}

// SAFETY: every call is forwarded unchanged to the system allocator; the counter is a
// const-initialised thread-local, which does not allocate.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        COUNT.with(|c| c.set(c.get().map(|n| n + 1)));
        // SAFETY: same contract as the caller's.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: same contract as the caller's.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        COUNT.with(|c| c.set(c.get().map(|n| n + 1)));
        // SAFETY: same contract as the caller's.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn allocations_per_run(
    pipeline: &mut Pipeline,
    segmenter: &mut dyn Segmenter,
    buffer: &mut [u8],
    format: PixelFormat,
) -> usize {
    let mut run = |i: u64| {
        pipeline
            .process(
                Frame::new(buffer, W as u32, H as u32, format),
                Duration::from_millis(33 * i),
                segmenter,
            )
            .expect("process");
    };
    for i in 0..3 {
        run(i);
    }
    COUNT.with(|c| c.set(Some(0)));
    for i in 3..13 {
        run(i);
    }
    COUNT.with(|c| c.replace(None)).unwrap_or_default()
}

#[test]
fn processing_frames_does_not_allocate() {
    let image = image::RgbImage::from_pixel(64, 64, image::Rgb([20, 120, 220]));
    let image = Background::Image(Arc::new(BackgroundImage::from_rgb(image)));
    let cases = [
        (
            Background::Blur { strength: 70 },
            Level::Full,
            ModelShape::Square,
        ),
        (
            Background::Blur { strength: 70 },
            Level::Light,
            ModelShape::Landscape,
        ),
        (image.clone(), Level::Full, ModelShape::Square),
        (image, Level::Light, ModelShape::Landscape),
    ];
    for (background, level, shape) in cases {
        for format in [PixelFormat::Nv12, PixelFormat::Rgba] {
            let mut buffer = match format {
                PixelFormat::Nv12 => frame(),
                _ => rgba_frame(),
            };
            let mut pipeline =
                Pipeline::new(W as u32, H as u32, background.clone()).expect("pipeline");
            pipeline.set_level(level);
            let mut segmenter = RectSegmenter(shape);
            let count = allocations_per_run(&mut pipeline, &mut segmenter, &mut buffer, format);
            assert_eq!(count, 0, "{level:?} {format:?}");
        }
    }
}
