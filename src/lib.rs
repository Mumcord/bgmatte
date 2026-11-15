//! Realtime background blur and background replacement for camera pictures.
//!
//! A person segmenter (the MediaPipe Selfie Segmenter, run by ONNX Runtime on the CPU) finds
//! the person in each frame. The pipeline refines the mask's edges against the picture, smooths
//! it over time and then blurs the background or replaces it with an image, in place, in the
//! frame's own buffer. Once warmed up, processing a frame allocates nothing.
//!
//! # Getting started
//!
//! [`Effects`] is the easiest entry point: it owns the pipeline and the models, and picks the
//! quality level ([`Quality::Auto`] measures what this machine can do and steps down to the
//! lighter level when it falls behind).
//!
//! ```no_run
//! use std::time::Duration;
//! use bgmatte::{Background, Config, Effects, Frame};
//!
//! # fn main() -> Result<(), bgmatte::Error> {
//! // Load ONNX Runtime from a known place. Without this call, `ORT_DYLIB_PATH` or the
//! // system's library search path is used.
//! bgmatte::init_runtime("/opt/onnxruntime/lib/libonnxruntime.so.1.23.2")?;
//!
//! let mut effects = Effects::new(Config::new(30, Background::Blur { strength: 60 }));
//! let (width, height) = (1280, 720);
//! let mut rgba = vec![0u8; width * height * 4];
//! // For every camera frame:
//! effects.process(
//!     Frame::rgba(&mut rgba, width as u32, height as u32),
//!     Duration::from_millis(33),
//! )?;
//! # Ok(())
//! # }
//! ```
//!
//! Frames are NV12 (processed in place without conversion, the format most webcams and video
//! encoders use), RGB or RGBA (8 bits per channel, rows without padding). Both sides must be
//! at least 64 pixels and divisible by 8 ([`Pipeline::supports`]).
//!
//! # Building blocks
//!
//! For more control, use the parts [`Effects`] is made of: a [`Pipeline`] per frame size, one
//! [`Segmenter`] per [`ModelShape`] ([`OnnxSegmenter`] or your own), and a
//! [`LevelController`] that turns measured frame times into a [`Level`].
//!
//! # ONNX Runtime
//!
//! The crate does not ship ONNX Runtime. It loads the library at run time, either from the path
//! given to [`init_runtime`] or, on first use, from `ORT_DYLIB_PATH` or the system's library
//! search path. When it can't be loaded, [`availability`] says why and the models report
//! [`Error::Unavailable`]; the frames are left as they are. ONNX Runtime 1.23 or newer is
//! needed; 1.23.2 is tested.

mod background;
mod color;
mod effects;
mod frame;
mod guided;
mod level;
mod ops;
mod pipeline;
mod runtime;
mod segmenter;

pub use background::{Background, BackgroundImage};
pub use effects::{Config, Effects, Stats};
pub use frame::{Frame, PixelFormat};
pub use level::{Level, LevelController, Load, Quality};
pub use pipeline::Pipeline;
pub use runtime::{availability, init_runtime};
pub use segmenter::{ModelShape, OnnxSegmenter, Segmenter};

/// Everything that can go wrong while setting up or applying a background effect.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// ONNX Runtime is missing or failed to load; the text says why.
    #[error("ONNX Runtime is not available: {0}")]
    Unavailable(String),
    /// The segmentation model could not be loaded or failed on a frame.
    #[error("segmentation failed: {0}")]
    Model(String),
    /// A background image could not be read or decoded.
    #[error("the background image can't be used: {0}")]
    Image(String),
    /// The frame's buffer is too small for its size and format, or the size differs from the
    /// one the [`Pipeline`] was made for.
    #[error("the frame buffer does not match the frame size and format")]
    FrameSize,
    /// Frames of this size can't be processed (see [`Pipeline::supports`]).
    #[error(
        "frames of {width}x{height} are not supported (both sides must be at least 64 and divisible by 8)"
    )]
    UnsupportedSize {
        /// Width of the frame in pixels.
        width: u32,
        /// Height of the frame in pixels.
        height: u32,
    },
}
