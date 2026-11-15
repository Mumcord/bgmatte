# Contributing

Thanks for helping. Timing reports are as valuable as code: see "Support" in the [README](README.md).

## Before you open a pull request

- Open an issue first for anything larger than a small fix, so the direction can be agreed.
- Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`, `cargo test` and `cargo doc --no-deps`. CI runs the same checks on Linux, Windows and macOS.
- The tests that run the real models are ignored without ONNX Runtime. Point `ORT_DYLIB_PATH` at ONNX Runtime 1.23.2 (see "ONNX Runtime" in the README) and run `cargo test -- --include-ignored`; CI does.
- Keep changes small and focused, one logical change per commit, with a clear message ([Conventional Commits](https://www.conventionalcommits.org/) style: `feat:`, `fix:`, `docs:`).
- New logic needs tests. A bug fix starts with a test that reproduces the bug.
- Add a line under `[Unreleased]` in `CHANGELOG.md` for every user-visible change.
- Public items need documentation; the documentation build must stay free of warnings.

## Code rules

- Nothing per frame may block, allocate, lock or log. `tests/no_alloc.rs` checks the allocations; keep it passing and extend it for new paths.
- No `unwrap` in library code; report errors through `Error` or log them.
- Comments explain why, not what.
- Changes to the models (`models/`) need the source, licence and checksum in `models/README.md` and `NOTICE`.

## Licence

By submitting a contribution you agree that it is licensed under `MIT OR Apache-2.0`, the same as the crate, without additional terms. The bundled models keep their Apache-2.0 licence.
