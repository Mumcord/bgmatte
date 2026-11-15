#!/usr/bin/env bash
# Downloads the official ONNX Runtime CPU release for this runner, checks it against the
# SHA-256 pinned below and prints the path of the library, for ORT_DYLIB_PATH:
#   fetch-onnxruntime.sh <directory>
# Used by CI to run the tests that need ONNX Runtime. The crate itself never downloads it.
set -euo pipefail

dir=${1:?usage: fetch-onnxruntime.sh <directory>}
version=1.23.2
base="https://github.com/microsoft/onnxruntime/releases/download/v$version"

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64)
    archive="onnxruntime-linux-x64-$version.tgz"
    sha256=1fa4dcaef22f6f7d5cd81b28c2800414350c10116f5fdd46a2160082551c5f9b
    library="onnxruntime-linux-x64-$version/lib/libonnxruntime.so.$version"
    ;;
  Linux-aarch64)
    archive="onnxruntime-linux-aarch64-$version.tgz"
    sha256=7c63c73560ed76b1fac6cff8204ffe34fe180e70d6582b5332ec094810241e5c
    library="onnxruntime-linux-aarch64-$version/lib/libonnxruntime.so.$version"
    ;;
  Darwin-arm64)
    archive="onnxruntime-osx-arm64-$version.tgz"
    sha256=b4d513ab2b26f088c66891dbbc1408166708773d7cc4163de7bdca0e9bbb7856
    library="onnxruntime-osx-arm64-$version/lib/libonnxruntime.$version.dylib"
    ;;
  Darwin-x86_64)
    archive="onnxruntime-osx-x86_64-$version.tgz"
    sha256=d10359e16347b57d9959f7e80a225a5b4a66ed7d7e007274a15cae86836485a6
    library="onnxruntime-osx-x86_64-$version/lib/libonnxruntime.$version.dylib"
    ;;
  MINGW*-x86_64 | MSYS*-x86_64)
    archive="onnxruntime-win-x64-$version.zip"
    sha256=0b38df9af21834e41e73d602d90db5cb06dbd1ca618948b8f1d66d607ac9f3cd
    library="onnxruntime-win-x64-$version/lib/onnxruntime.dll"
    ;;
  *)
    echo "no pinned ONNX Runtime build for $(uname -s)-$(uname -m)" >&2
    exit 1
    ;;
esac

# Windows: a path both Git Bash and Windows programs (the tests) understand.
if command -v cygpath > /dev/null; then
  dir=$(cygpath -m "$dir")
fi
mkdir -p "$dir"
curl --silent --show-error --fail --location --retry 3 --output "$dir/$archive" "$base/$archive"
if command -v sha256sum > /dev/null; then
  actual=$(sha256sum "$dir/$archive" | cut -d ' ' -f 1)
else
  actual=$(shasum -a 256 "$dir/$archive" | cut -d ' ' -f 1)
fi
if [[ "$actual" != "$sha256" ]]; then
  echo "$archive has SHA-256 $actual, expected $sha256" >&2
  exit 1
fi
case "$archive" in
  *.zip) 7z x -y -o"$dir" "$dir/$archive" > /dev/null ;;
  *) tar -xzf "$dir/$archive" -C "$dir" ;;
esac
if [[ ! -f "$dir/$library" ]]; then
  echo "$archive does not contain $library" >&2
  exit 1
fi
echo "$dir/$library"
