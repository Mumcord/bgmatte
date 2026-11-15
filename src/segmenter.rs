//! Person segmentation: the MediaPipe Selfie Segmenter run by ONNX Runtime, behind a trait so
//! that tests and applications can supply their own masks.

use std::time::Duration;

use ort::session::Session;
use ort::session::builder::GraphOptimizationLevel;
use ort::value::TensorRef;

use crate::Error;

/// Square 256×256 model for the full level.
static SQUARE_MODEL: &[u8] = include_bytes!("../models/selfie_segmenter_square.onnx");
/// Landscape 256×144 model for the light level.
static LANDSCAPE_MODEL: &[u8] = include_bytes!("../models/selfie_segmenter_landscape.onnx");

/// One CPU thread for the model: next to capture and encoding, a camera should stay within
/// about 1.5 cores, and a second thread gains little at these input sizes.
const MODEL_THREADS: usize = 1;

/// Which of the two bundled models to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModelShape {
    /// 256×256 input, used at [`Level::Full`](crate::Level::Full).
    Square,
    /// 256×144 input, used at [`Level::Light`](crate::Level::Light).
    Landscape,
}

impl ModelShape {
    /// Input width and height in pixels.
    pub fn size(self) -> (usize, usize) {
        match self {
            Self::Square => (256, 256),
            Self::Landscape => (256, 144),
        }
    }
}

/// Produces a person mask from a small picture of the frame.
///
/// Implement it to use another model or a known mask (tests, recorded masks).
pub trait Segmenter: Send {
    /// The input size this segmenter expects.
    fn shape(&self) -> ModelShape;

    /// Writes the person mask for `rgb` into `mask`.
    ///
    /// `rgb` is planar (`3 × h × w`: all red values, then green, then blue) with values in
    /// `0.0..=1.0`, at the size of [`shape`](Self::shape). `mask` has `h × w` values to fill
    /// with `0.0` (background) … `1.0` (person). `timestamp` is the frame's timestamp, for
    /// segmenters that replay known masks.
    ///
    /// # Errors
    ///
    /// Any error; the frame is then left unchanged.
    fn segment(&mut self, rgb: &[f32], timestamp: Duration, mask: &mut [f32]) -> Result<(), Error>;
}

/// The bundled Selfie Segmenter run by ONNX Runtime on the CPU.
pub struct OnnxSegmenter {
    shape: ModelShape,
    session: Session,
}

impl std::fmt::Debug for OnnxSegmenter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OnnxSegmenter")
            .field("shape", &self.shape)
            .finish_non_exhaustive()
    }
}

impl OnnxSegmenter {
    /// Loads the model of `shape`.
    ///
    /// # Errors
    ///
    /// [`Error::Unavailable`] when ONNX Runtime can't be loaded (see
    /// [`availability`](crate::availability)), [`Error::Model`] when the session can't be
    /// created.
    pub fn new(shape: ModelShape) -> Result<Self, Error> {
        let environment = crate::runtime::environment()?;
        let model = match shape {
            ModelShape::Square => SQUARE_MODEL,
            ModelShape::Landscape => LANDSCAPE_MODEL,
        };
        let session = build_session(model, environment).map_err(Error::Model)?;
        Ok(Self { shape, session })
    }
}

fn build_session(
    model: &[u8],
    environment: &ort::environment::Environment,
) -> Result<Session, String> {
    Session::builder(environment)
        .map_err(|e| e.to_string())?
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .map_err(|e| e.to_string())?
        .with_intra_threads(MODEL_THREADS)
        .map_err(|e| e.to_string())?
        .with_inter_threads(1)
        .map_err(|e| e.to_string())?
        // Spinning worker threads would burn a core between frames.
        .with_intra_op_spinning(false)
        .map_err(|e| e.to_string())?
        .commit_from_memory(model)
        .map_err(|e| e.to_string())
}

impl Segmenter for OnnxSegmenter {
    fn shape(&self) -> ModelShape {
        self.shape
    }

    fn segment(
        &mut self,
        rgb: &[f32],
        _timestamp: Duration,
        mask: &mut [f32],
    ) -> Result<(), Error> {
        let (w, h) = self.shape.size();
        let input = TensorRef::from_array_view(([1usize, 3, h, w], rgb))
            .map_err(|err| Error::Model(err.to_string()))?;
        let outputs = self
            .session
            .run(ort::inputs!["pixel_values" => input])
            .map_err(|err| Error::Model(err.to_string()))?;
        let (_, alphas) = outputs["alphas"]
            .try_extract_tensor::<f32>()
            .map_err(|err| Error::Model(err.to_string()))?;
        if alphas.len() != mask.len() {
            return Err(Error::Model(format!(
                "unexpected mask size {}",
                alphas.len()
            )));
        }
        mask.copy_from_slice(alphas);
        Ok(())
    }
}
