# Changelog

All notable changes to this project are listed here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project follows [Semantic Versioning](https://semver.org/). Below 1.0, interface changes may happen in any release and are listed here.

## [Unreleased]

## [0.1.0] - 2026-11-15

First release.

### Added

- Person segmentation with the MediaPipe Selfie Segmenter (square 256×256 and landscape 256×144), run by ONNX Runtime on the CPU (`OnnxSegmenter`). Both models are bundled in the crate under Apache-2.0; nothing is downloaded at build or run time.
- ONNX Runtime loaded at run time: `init_runtime` with an explicit path, otherwise `ORT_DYLIB_PATH` or the system's library search path; `availability` says why it is missing. Without it, effects are off and frames pass unchanged.
- `Pipeline`: edge refinement with a fast guided filter, motion-weighted temporal smoothing, background blur (strength 0–100) and background images with light wrap. No allocation per frame once warmed up.
- Frames as NV12 (processed in place), RGB and RGBA (`Frame`, `PixelFormat`); unchanged pixels keep their exact values.
- Quality levels Auto, Best and Light (`Quality`, `Level`) with the load monitor `LevelController`: Auto measures the full level on the first frames, steps down after 3 s over budget or on battery, and back up after 30 s under budget.
- `Effects`: the complete effect for one camera, frame in and frame out, with models created on demand, background and quality changes while running, and `Stats` (level, load, median time, too busy, unavailable reason). `Effects::with_segmenters` takes custom segmenters.
- `Segmenter` trait for own models or known masks.
- `BackgroundImage` decodes PNG, JPEG and WebP with size limits for untrusted files.
- Examples `replace_background` (blur or replace the background of a photo) and `bench` (time per 720p frame).

### Changed

- Extracted from the Mumcord `crates/effects` crate and relicensed under `MIT OR Apache-2.0`. Camera capture, the effects thread, settings and test hooks stayed in Mumcord.
- `Pipeline::process` takes a `Frame` instead of an NV12 slice, and `Pipeline::new` returns an error for unsupported frame sizes.
- The quality setting is `bgmatte::Quality` instead of Mumcord's settings type.
