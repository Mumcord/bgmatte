//! The complete effect for one camera: pipeline, models and quality level in one type.

use std::time::{Duration, Instant};

use crate::Error;
use crate::background::Background;
use crate::frame::Frame;
use crate::level::{Level, LevelController, Load, Quality};
use crate::pipeline::Pipeline;
use crate::segmenter::{ModelShape, OnnxSegmenter, Segmenter};

type SegmenterFactory = Box<dyn FnMut(ModelShape) -> Result<Box<dyn Segmenter>, Error> + Send>;

/// Settings for [`Effects`].
#[derive(Debug, Clone)]
pub struct Config {
    /// Frame rate of the camera; sets the time budget per frame.
    pub fps: u32,
    /// The quality setting.
    pub quality: Quality,
    /// What goes behind the person.
    pub background: Background,
    /// Whether the machine runs on battery (Auto then stays at the light level).
    pub on_battery: bool,
}

impl Config {
    /// [`Quality::Auto`] on mains power.
    pub fn new(fps: u32, background: Background) -> Self {
        Self {
            fps,
            quality: Quality::Auto,
            background,
            on_battery: false,
        }
    }
}

/// What the effect is doing, for example to show next to a camera preview.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stats {
    /// The running level; `None` while no effect runs (background None, or unavailable).
    pub level: Option<Level>,
    /// How much of the frame budget the effect uses.
    pub load: Load,
    /// Median effect time per frame over the last frames.
    pub median: Duration,
    /// The effect can't even hold 15 fps on this machine.
    pub too_busy: bool,
    /// Why the effect is off on this machine, when ONNX Runtime is unavailable.
    pub unavailable: Option<String>,
    /// Frames the effect was applied to so far.
    pub frames: u64,
}

/// The background effect for one camera, frame in, frame out.
///
/// Creates the models when they are first needed, follows the [`Quality`] setting with a
/// [`LevelController`], and makes a new [`Pipeline`] when the frame size changes. Processing
/// a frame allocates nothing once both are set up.
///
/// It runs on the caller's thread. A camera pipeline usually runs it on a thread of its own
/// between capture and encoder, and drops frames that arrive while one is still being
/// processed.
pub struct Effects {
    fps: u32,
    quality: Quality,
    background: Background,
    on_battery: bool,
    controller: LevelController,
    pipeline: Option<Pipeline>,
    full: Option<Box<dyn Segmenter>>,
    light: Option<Box<dyn Segmenter>>,
    segmenters: SegmenterFactory,
    unavailable: Option<String>,
    frames: u64,
}

impl std::fmt::Debug for Effects {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Effects")
            .field("quality", &self.quality)
            .field("background", &self.background)
            .field("controller", &self.controller)
            .field("unavailable", &self.unavailable)
            .finish_non_exhaustive()
    }
}

impl Effects {
    /// The effect with the bundled models ([`OnnxSegmenter`]). ONNX Runtime is loaded when the
    /// first frame needs a model (see [`init_runtime`](crate::init_runtime)).
    pub fn new(config: Config) -> Self {
        Self::with_segmenters(config, |shape| {
            Ok(Box::new(OnnxSegmenter::new(shape)?) as Box<dyn Segmenter>)
        })
    }

    /// The effect with segmenters made by `segmenters`, called once per [`ModelShape`] when
    /// it is first needed. Returning [`Error::Unavailable`] turns the effect off for good (see
    /// [`Stats::unavailable`]); other errors are returned from [`process`](Self::process) and
    /// tried again on the next frame.
    pub fn with_segmenters(
        config: Config,
        segmenters: impl FnMut(ModelShape) -> Result<Box<dyn Segmenter>, Error> + Send + 'static,
    ) -> Self {
        Self {
            fps: config.fps,
            quality: config.quality,
            background: config.background,
            on_battery: config.on_battery,
            controller: LevelController::new(config.quality, config.fps, config.on_battery),
            pipeline: None,
            full: None,
            light: None,
            segmenters: Box::new(segmenters),
            unavailable: None,
            frames: 0,
        }
    }

    /// Changes the background; takes effect with the next frame. An image is scaled to the
    /// frame here, not per frame.
    pub fn set_background(&mut self, background: Background) {
        if let Some(pipeline) = &mut self.pipeline {
            pipeline.set_background(background.clone());
        }
        self.background = background;
    }

    /// Changes the quality setting; Auto measures again.
    pub fn set_quality(&mut self, quality: Quality) {
        self.quality = quality;
        self.controller = LevelController::new(quality, self.fps, self.on_battery);
    }

    /// Tells the effect whether the machine runs on battery. Reading the power state can be
    /// slow, so it is left to the caller (every few seconds is enough).
    pub fn set_on_battery(&mut self, on_battery: bool) {
        self.on_battery = on_battery;
    }

    /// Applies the effect to `frame` in place. `timestamp` is the frame's capture time.
    ///
    /// With [`Background::None`], or once ONNX Runtime turned out to be unavailable, frames
    /// pass unchanged and `Ok` is returned.
    ///
    /// # Errors
    ///
    /// The frame is left unchanged on every error:
    /// - [`Error::UnsupportedSize`] for frame sizes the pipeline can't handle;
    /// - [`Error::Unavailable`] once, when the model can't be created because ONNX Runtime is
    ///   missing (afterwards see [`Stats::unavailable`]);
    /// - [`Error::Model`] or [`Error::FrameSize`] for a failed frame.
    pub fn process(&mut self, frame: Frame<'_>, timestamp: Duration) -> Result<(), Error> {
        if self.background.is_none() || self.unavailable.is_some() {
            return Ok(());
        }
        let size = (frame.width(), frame.height());
        let level = self.controller.level();
        let pipeline = match &mut self.pipeline {
            Some(pipeline) if pipeline.size() == size => pipeline,
            slot => {
                let mut pipeline = Pipeline::new(size.0, size.1, self.background.clone())?;
                pipeline.set_level(level);
                slot.insert(pipeline)
            }
        };
        let (slot, shape) = match level {
            Level::Full => (&mut self.full, ModelShape::Square),
            Level::Light => (&mut self.light, ModelShape::Landscape),
        };
        let segmenter = match slot {
            Some(segmenter) => segmenter,
            None => match (self.segmenters)(shape) {
                Ok(segmenter) => slot.insert(segmenter),
                Err(Error::Unavailable(reason)) => {
                    log::warn!("background effects unavailable: {reason}");
                    self.unavailable = Some(reason.clone());
                    return Err(Error::Unavailable(reason));
                }
                Err(err) => return Err(err),
            },
        };
        let started = Instant::now();
        pipeline.process(frame, timestamp, segmenter.as_mut())?;
        let elapsed = started.elapsed();
        self.frames += 1;
        if let Some(level) = self
            .controller
            .record(Instant::now(), elapsed, self.on_battery)
        {
            log::info!("background effect level changed to {level:?}");
            pipeline.set_level(level);
        }
        Ok(())
    }

    /// What the effect is doing now.
    pub fn stats(&self) -> Stats {
        let running = !self.background.is_none() && self.unavailable.is_none();
        Stats {
            level: running.then(|| self.controller.level()),
            load: if running {
                self.controller.load()
            } else {
                Load::Low
            },
            median: self.controller.median(),
            too_busy: running && self.controller.too_busy(),
            unavailable: self.unavailable.clone(),
            frames: self.frames,
        }
    }
}
