# Build and dependencies

## Desktop build

The current native desktop backend targets macOS and Metal. Install Rust 1.95 or later,
Apple Command Line Tools with the macOS SDK, and a working GPU driver. Cargo resolves
Rust dependencies from the workspace manifests and `Cargo.lock`.

```sh
cargo build --release
cargo run --release
# Open a sample and its Program window
cargo run --release -- --project sample/10-dual-projector.vjay.json --output-window
```

`target/release/vjay --help` lists command-line options. Without `--project`, startup uses
`vjay-project.json` if present. The GUI supports arbitrary project files through Open,
Save, and Save As; it is not limited to that default path.

FFmpeg and FFprobe are external runtime tools. Put both on `PATH`, or select them explicitly:

```sh
cargo run --release -- --ffmpeg /path/to/ffmpeg --ffprobe /path/to/ffprobe
```

Video and audio codec support depends on the installed FFmpeg build. On macOS, playback
requires VideoToolbox and `scale_vt` support. Unsupported source formats may need the
hardware-encoded compatibility cache described in the [user guide](USER_GUIDE.md#video-and-audio).
The procedural [sample projects](SAMPLES.md) need no external media; Text uses installed fonts.

## URL video imports

Install `yt-dlp` and its default dependencies using the pinned requirements. Full YouTube
format support also needs EJS and a supported JavaScript runtime; Deno on `PATH` was used
for validation.

```sh
python3 -m venv .venv-web
.venv-web/bin/python -m pip install -r requirements-web.txt
cargo run --release -- --yt-dlp /path/to/yt-dlp
```

Startup prefers the repository's `.venv-web/bin/yt-dlp`, then `PATH`. The importer downloads
a local cache; it does not embed a web browser or capture an arbitrary page.

## NDI Runtime

Install the NDI Runtime separately. VJAY loads it dynamically and does not bundle it.
On macOS the default path is `/usr/local/lib/libndi.dylib`. Set `NDI_RUNTIME_PATH` to use
another library path. An unavailable Runtime disables that adapter without preventing startup.

NDI OUT uses the approved asynchronous GPU-to-CPU transfer exception. Input decoding
hardware use depends on the Runtime and codec. See [architecture](ARCHITECTURE.md#video-sharing-boundary-and-the-readback-exception).

## Syphon on macOS

Python 3 and Apple Command Line Tools can build the pinned upstream framework locally:

```sh
python3 scripts/build_syphon.py
cargo run --release
```

The script downloads sources into `.dependencies/`, builds under `target/`, and does not
install into system locations. The default library is
`target/syphon/Syphon.framework/Syphon`. `SYPHON_FRAMEWORK_PATH` may point to a framework
directory or its binary.

Metal texture sharing and GPU scaling have passed same-process SDK loopback tests.
Long-running interoperability and reconnection tests with other vendors' applications
have not been performed.

## Spout on Windows

The adapter uses D3D12/D3D11On12 and supports same-GPU 8-bit RGBA/BGRA sharing.
Build its bridge with Visual Studio C++, the Windows SDK, and CMake:

```powershell
cmake -S native/spout -B target/spout -A x64
cmake --build target/spout --config Release
$env:SPOUT_BRIDGE_PATH = (Resolve-Path target/spout/Release/vjay_spout_bridge.dll)
cargo run --release
```

The Windows bridge and native sender/receiver have not been built or tested on a Windows
machine in this workspace. Windows desktop presentations are not implemented; this adapter
is not a claim of a complete Windows desktop build. Use Syphon on macOS.

## Platform scope

| Component | Validation or limitation |
| --- | --- |
| GPUI + wgpu native previews and output windows | Validated on macOS / Apple M2 Pro / Metal |
| VideoToolbox decode and GPU scaling | Validated for supported macOS codecs |
| Linux / Windows native presentation and GPU-decode PALs | Not implemented |
| Portable FFmpeg decoder | CPU-based; not the selected macOS playback policy |
| MIDI / OSC / Link | Native and virtual-input tests exist; physical controller coverage is limited |
| Long performances and multiple physical projectors | Not validated by headless or loopback tests |

## Verification

Run the workspace checks after code changes:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Run native tests explicitly when the required GPU, devices, Runtime, and tools are available.
The audio tests use zero gain. Some tests read an installed rekordbox library or use
network services; inspect each ignored test's requirements.

```sh
# Core GPU validation
cargo test -p vjay-infrastructure --test gpu -- --ignored --test-threads=1
# All ten samples, including both projector outputs
cargo test -p vjay-infrastructure --test samples -- --include-ignored --test-threads=1
# Full native suite: configure external dependencies first
VJAY_TEST_VIDEO=/path/to/4k.mov \
VJAY_TEST_YTDLP=/path/to/yt-dlp \
VJAY_TEST_URL=https://www.youtube.com/watch?v=jNQXAC9IVRw \
  cargo test --workspace -- --ignored --test-threads=1
```

For a deterministic headless capture:

```sh
cargo run --release -- --headless --project sample/03-liquid-contours.vjay.json \
  --frames 120 --size 1280x720 --output /tmp/vjay.ppm
```

Only the final image is exported. Headless runs do not open an audio device.
Reported loop time plus capture is not a measurement of sustained live playback FPS.

If `target/debug/` grows large, remove it after verification and only while Cargo is idle.
Keep `target/release/` and any needed `target/syphon/` framework available.

## Licenses

VJAY's project source uses the [MIT license](../LICENSE). Third-party code, fonts,
executables, and SDKs retain their own terms; see [dependency notices](THIRD_PARTY.md).
