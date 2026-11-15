# Models

The two person segmentation models bundled with bgmatte. They are compiled into the crate, so
nothing is downloaded at build or run time.

| File | Input | Used at | SHA-256 |
| --- | --- | --- | --- |
| `selfie_segmenter_square.onnx` | 256×256 | `Level::Full` | `3241ac4ad8aa35bdaf33946776db29f7c283a413aa0b0dacb9483594b4531aad` |
| `selfie_segmenter_landscape.onnx` | 256×144 | `Level::Light` | `7a0adcfdb1715d3b0ff0f61486d8a181c33f4a208343a5abfccbc6540e872d24` |

## Origin and licence

Both are the **MediaPipe Selfie Segmenter** by Google (authors Tingbo Hou, Siargey Pisarchyk,
Karthik Raveendran; model card dated May 6, 2021), licensed under the
[Apache License, Version 2.0](../LICENSE-APACHE). See the
[model card](https://storage.googleapis.com/mediapipe-assets/Model%20Card%20MediaPipe%20Selfie%20Segmentation.pdf)
for intended use, limitations and fairness evaluation, and the
[MediaPipe image segmenter page](https://ai.google.dev/edge/mediapipe/solutions/vision/image_segmenter)
for the model family.

The files here are the ONNX conversions published by the ONNX Community on Hugging Face, also
under Apache-2.0, unchanged and pinned to these revisions:

- Square: <https://huggingface.co/onnx-community/mediapipe_selfie_segmentation/resolve/be49485c8e027524be38591817fc5cd31bd9d00e/onnx/model.onnx>
- Landscape: <https://huggingface.co/onnx-community/mediapipe_selfie_segmentation_landscape/resolve/2497d5bec26c626c7b3c4edc6e1fefc21b64f6c3/onnx/model.onnx>

Each model takes `pixel_values` (planar RGB, `1 × 3 × h × w`, values 0..1) and returns
`alphas` (`1 × 1 × h × w`, 0 = background, 1 = person).

To check the files: `sha256sum models/*.onnx`.
