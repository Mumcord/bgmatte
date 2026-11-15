# bgmatte

Realtime background blur and background replacement for camera pictures in Rust.

- **Person segmentation** with the MediaPipe Selfie Segmenter, run by ONNX Runtime on the CPU. Both models are bundled; nothing is downloaded at build or run time.
- **Clean edges:** a guided filter snaps the mask to hair and shoulders in the full-resolution picture, and motion-weighted temporal smoothing removes flicker without ghosting.
- **Effects:** background blur (adjustable strength) and background images, with a light wrap that lets the background's colour touch the person's edge.
- **Quality levels** Auto, Best and Light. Auto measures what the machine can do and steps down when it falls behind or runs on battery.
- **Frame in, frame out:** NV12 (in place, no conversion), RGB or RGBA. Once warmed up, processing a frame allocates nothing.

About 8 ms per 720p frame at the full level and under 5 ms at the light level on one CPU core (see [Performance](#performance)).

Licence: `MIT OR Apache-2.0`. Minimum Rust version: 1.92.

> **Status: 0.1, best effort.** Extracted from [Mumcord](https://github.com/Mumcord/mumcord), where it runs in camera calls. It was measured on few machines; reports are welcome, see [Support](#support).

## Quick start

```toml
[dependencies]
bgmatte = "0.1"
```

bgmatte needs ONNX Runtime at run time, see [ONNX Runtime](#onnx-runtime). Blur the background of every camera frame:

```rust
use std::time::Duration;
use bgmatte::{Background, Config, Effects, Frame};

fn main() -> Result<(), bgmatte::Error> {
    // Optional: without this, ORT_DYLIB_PATH or the system's library search path is used.
    bgmatte::init_runtime("/opt/onnxruntime/lib/libonnxruntime.so.1.23.2")?;

    let mut effects = Effects::new(Config::new(30, Background::Blur { strength: 60 }));
    let (width, height) = (1280, 720);
    let mut rgba = vec![0u8; width * height * 4];
    // For each camera frame, in order:
    effects.process(Frame::rgba(&mut rgba, width as u32, height as u32), Duration::from_millis(33))?;
    println!("{:?}", effects.stats());
    Ok(())
}
```

Replace the background with a picture instead:

```rust
use std::sync::Arc;
use bgmatte::{Background, BackgroundImage, Effects};

fn use_beach(effects: &mut Effects) -> Result<(), bgmatte::Error> {
    let beach = BackgroundImage::open("beach.jpg")?;
    effects.set_background(Background::Image(Arc::new(beach)));
    Ok(())
}
```

`Effects::set_background`, `set_quality` and `set_on_battery` change the effect while frames keep coming. `Effects::stats` reports the running level, the load, the median time per frame, whether the machine is too busy for 15 fps, and why the effect is off if ONNX Runtime is missing.

Two examples in `examples/` show complete programs. Both need ONNX Runtime (`ORT_DYLIB_PATH`):

- `cargo run --release --example replace_background -- photo.jpg out.png blur=80` blurs the background of a photo; pass an image file instead of `blur=80` to replace it.
- `cargo run --release --example bench -- [photo]` measures the time per 720p frame for each level and background.

The API documentation (`cargo doc --open`) covers the rest.

### Building blocks

`Effects` is made of parts you can use directly: a `Pipeline` per frame size that does the work on one frame, a `Segmenter` per `ModelShape` (`OnnxSegmenter`, or your own implementation, for example a recorded mask in tests), and a `LevelController` that turns measured frame times into a `Level`. `Effects::with_segmenters` keeps the level logic but takes your segmenters.

## Frames

| Format | Layout | How it is processed |
| --- | --- | --- |
| NV12 | Y plane, then interleaved U/V at half resolution, BT.601 limited range | In place |
| RGB | 3 bytes per pixel | Converted to NV12, processed, changed pixels converted back |
| RGBA | 4 bytes per pixel | Like RGB; alpha is left untouched |

Rows must be tightly packed. Both sides must be at least 64 pixels and divisible by 8. Pixels the effect does not change (the person) keep their exact values in every format.

Feed the frames of one camera in order with their capture timestamps: the temporal smoothing keeps history from frame to frame, and the light level runs the model at 15 fps and reuses its mask in between.

## Quality levels

| Setting | Behaviour |
| --- | --- |
| `Quality::Best` | Always the full level: square model on every frame, guided-filter edges, light wrap for images, finer blur. |
| `Quality::Light` | Always the light level: landscape model at 15 fps, simpler edges, coarser blur. About half the work. |
| `Quality::Auto` | Measures the full level on the first frames and keeps it if it fits 60 % of the frame interval. Steps down after 3 s over budget or on battery, and back up after 30 s under budget on mains power. |

## ONNX Runtime

bgmatte does not ship ONNX Runtime and never downloads it. Your application provides the library and bgmatte loads it at run time (the `load-dynamic` mode of the [`ort`](https://crates.io/crates/ort) crate), so a program still starts without it: the effects then report `Error::Unavailable` with the reason, and frames pass unchanged.

- **Version:** 1.23 or newer. Tested with **1.23.2**, the last release with an Intel macOS build.
- **Where to get it:** the official CPU builds from [Microsoft's ONNX Runtime releases](https://github.com/microsoft/onnxruntime/releases/tag/v1.23.2), for example `onnxruntime-linux-x64-1.23.2.tgz`, `onnxruntime-win-x64-1.23.2.zip` or `onnxruntime-osx-arm64-1.23.2.tgz`. The library is in the archive's `lib/` folder. ONNX Runtime is MIT-licensed; ship its licence with your application.
- **How bgmatte finds it**, in this order:
  1. `bgmatte::init_runtime(path)` with the library's path, for example next to your executable. Call it before the first frame.
  2. The environment variable `ORT_DYLIB_PATH`.
  3. The platform's library name (`onnxruntime.dll`, `libonnxruntime.so`, `libonnxruntime.dylib`) on the system's library search path.

ONNX Runtime is loaded once per process; the first attempt decides. `bgmatte::availability()` tells you whether it worked and why not.

The models run on one CPU thread with spinning worker threads switched off, to leave room for capture and encoding.

## Models and licences

The two bundled models are the [MediaPipe Selfie Segmenter](https://ai.google.dev/edge/mediapipe/solutions/vision/image_segmenter) by Google, in the ONNX conversions of the [ONNX Community](https://huggingface.co/onnx-community/mediapipe_selfie_segmentation):

| Model | Input | Used at | Licence |
| --- | --- | --- | --- |
| Selfie Segmenter (square) | 256×256 | Full level | Apache-2.0 |
| Selfie Segmenter (landscape) | 256×144 | Light level | Apache-2.0 |

The model card ([PDF](https://storage.googleapis.com/mediapipe-assets/Model%20Card%20MediaPipe%20Selfie%20Segmentation.pdf)) states the Apache License 2.0 and describes intended use and limitations: the models are made for one or two people close to the camera, as in video calls. Origin, revisions and checksums are in [models/README.md](models/README.md); attribution is in [NOTICE](NOTICE). Applications that ship bgmatte should ship the NOTICE file too.

## Performance

Median time per 1280×720 frame from `cargo run --release --example bench` on an AMD Ryzen 7 5800X3D (one core for the model), with ONNX Runtime 1.23.2 on Linux:

| Case | Median | Model share |
| --- | --- | --- |
| Full level, blur | 8.3 ms | 3.5 ms |
| Full level, image | 6.5 ms | 3.5 ms |
| Light level, blur | 4.7 ms | 0.8 ms |
| Light level, image | 2.9 ms | 0.8 ms |
| Full level, blur, RGBA frame | 16.3 ms | 3.5 ms |

Most work happens at quarter resolution; only the final blend touches every pixel. RGB and RGBA frames cost more than NV12 because of the colour conversion in both directions.

## Limits

- CPU only. GPU execution providers of ONNX Runtime are not used in 0.1.
- Made for video calls: one or two people near the camera. Several people far from the camera, or no person at all, give weak masks.
- Frame sides must be divisible by 8; rows can't have padding.
- RGB and RGBA frames take about twice the time of NV12.

## Support

Best effort. Issues and pull requests are read, but there is no guaranteed response time. Timing reports from `cargo run --release --example bench` with your CPU and operating system are welcome in an issue.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## Licence

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or [MIT licence](LICENSE-MIT), at your option.

Contributions are accepted under `MIT OR Apache-2.0`, without additional terms.

The bundled models keep their own licence:

| Component | Licence |
| --- | --- |
| MediaPipe Selfie Segmenter models (`models/`) | Apache-2.0 |
