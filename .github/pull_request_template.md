## What changes and why

## Checklist

- [ ] `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`, `cargo test` and `cargo doc --no-deps` pass
- [ ] With ONNX Runtime available (`ORT_DYLIB_PATH`), `cargo test -- --include-ignored` passes
- [ ] New logic has tests; a bug fix starts with a test that reproduces the bug
- [ ] The per-frame path still allocates nothing (`tests/no_alloc.rs`)
- [ ] `CHANGELOG.md` has a line under `[Unreleased]` for every user-visible change
- [ ] I agree that my contribution is licensed under `MIT OR Apache-2.0` (see CONTRIBUTING.md)
