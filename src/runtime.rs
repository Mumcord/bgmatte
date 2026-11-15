//! Loading ONNX Runtime at run time.
//!
//! The library can be loaded only once per process, so the first attempt decides: either an
//! explicit [`init_runtime`] or, on first use of a model, the default place. All models share
//! the one environment kept here.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use ort::environment::Environment;

use crate::Error;

static RUNTIME: OnceLock<Result<Environment, String>> = OnceLock::new();

/// Loads ONNX Runtime from `path` (for example `libonnxruntime.so.1.23.2`, `onnxruntime.dll`
/// or `libonnxruntime.1.23.2.dylib`).
///
/// Call it before the first model is created. ONNX Runtime can be loaded only once per
/// process: when it was already loaded (or failed to load), this returns that first result and
/// ignores `path`.
///
/// # Errors
///
/// [`Error::Unavailable`] when the library is missing, too old (1.23 or newer is needed) or
/// fails to initialise.
pub fn init_runtime(path: impl AsRef<Path>) -> Result<(), Error> {
    let path = path.as_ref();
    RUNTIME
        .get_or_init(|| load(path))
        .as_ref()
        .map(|_| ())
        .map_err(|reason| Error::Unavailable(reason.clone()))
}

/// Whether the models can run in this process; the error says why not. Loads ONNX Runtime from
/// the default place when [`init_runtime`] was not called: the path in the environment
/// variable `ORT_DYLIB_PATH`, otherwise the platform's library name (`onnxruntime.dll`,
/// `libonnxruntime.so`, `libonnxruntime.dylib`) on the system's library search path.
///
/// # Errors
///
/// [`Error::Unavailable`] with the reason.
pub fn availability() -> Result<(), Error> {
    environment().map(|_| ())
}

pub(crate) fn environment() -> Result<&'static Environment, Error> {
    RUNTIME
        .get_or_init(|| load(&default_path()))
        .as_ref()
        .map_err(|reason| Error::Unavailable(reason.clone()))
}

fn default_path() -> PathBuf {
    match std::env::var_os("ORT_DYLIB_PATH") {
        Some(path) if !path.is_empty() => PathBuf::from(path),
        _ => PathBuf::from(format!(
            "{}onnxruntime{}",
            std::env::consts::DLL_PREFIX,
            std::env::consts::DLL_SUFFIX
        )),
    }
}

fn load(path: &Path) -> Result<Environment, String> {
    let builder = ort::init_from(path).map_err(|err| {
        log::warn!("ONNX Runtime failed to load from {}: {err}", path.display());
        format!("the ONNX Runtime library failed to load ({err})")
    })?;
    let environment = builder.with_name("bgmatte").build().map_err(|err| {
        log::warn!("ONNX Runtime environment failed to initialise: {err}");
        format!("the ONNX Runtime environment failed to initialise ({err})")
    })?;
    log::info!("ONNX Runtime loaded from {}", path.display());
    Ok(environment)
}
