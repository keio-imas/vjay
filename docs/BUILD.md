# Build and dependencies

## Desktop build

Desktop presentation adapters target macOS / Metal and Windows / DirectX 12. Install
Rust 1.95 or later and the platform tools below. Cargo resolves Rust dependencies from
the workspace manifests and `Cargo.lock`. macOS requires Apple Command Line Tools
with the macOS SDK and a working Metal GPU.

The frontend uses `gpui-pre` and `gpui-pre-platform`, both pinned to `=0.3.13`
(Zed revision `125ca8672e928b19a62c2e4b761fb29fe2a34ae3`). These are Longbridge's
version-aligned snapshots of upstream GPUI. Update the pair together and retain
`Cargo.lock`; snapshot updates may require API changes. The platform adapter selects
the native backend through `gpui_platform::application()`. Its `font-kit` feature
provides macOS glyph rasterization, and `runtime_shaders` retains support for Apple
Command Line Tools without a full Xcode installation.

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
uses native VideoToolbox decoding and Metal conversion. The comparative/portable FFmpeg
adapter additionally requires `scale_vt` for its hardware mode. Unsupported source formats may need the
hardware-encoded compatibility cache described in the [user guide](USER_GUIDE.md#video-and-audio).
The procedural [sample projects](SAMPLES.md) need no external media; Text uses installed fonts.

## Windows source-build prerequisites

Use 64-bit Rust with the `x86_64-pc-windows-msvc` target, Visual Studio C++ Build Tools
(including a Windows SDK with the FXC shader compiler), CMake, and a Windows-native Perl on `PATH`. Run the build
from an x64 Developer PowerShell / Native Tools environment.

The workspace enables `rusqlite`'s `bundled-sqlcipher-vendored-openssl` feature for the
rekordbox adapter. It builds SQLCipher and OpenSSL from source rather than requiring a
system OpenSSL installation. OpenSSL's Configure step needs Perl even when assembly is
disabled. A missing `perl` command produces the `openssl-sys` error before Rust code is
compiled. Keep SQLCipher enabled; replacing it with ordinary SQLite would break encrypted
rekordbox database support.

Install [Strawberry Perl](https://strawberryperl.com/), the Windows distribution
[recommended by OpenSSL for MSVC](https://github.com/openssl/openssl/blob/master/NOTES-PERL.md):

```powershell
winget install --exact --id StrawberryPerl.StrawberryPerl
```

Open a new x64 Developer PowerShell after installation so it inherits the updated PATH:

```powershell
Get-Command perl
perl -v
Get-Command cl, nmake, cmake
rustc -vV
```

If Perl is installed at its default location but the current shell has an old PATH,
set it for that shell instead of reinstalling:

```powershell
$env:Path = "C:\Strawberry\perl\bin;" + $env:Path
perl -v
cargo build --release --locked
```

Use the actual installation directory if it differs. `no-asm` in OpenSSL's Configure
arguments means NASM is not required for that build. This resolves the reported missing
Perl prerequisite; it is not confirmation of a complete Windows build or functioning
native VJ output. Windows has a DX12 presentation PAL; its native GPU movie decoder
and camera input remain unimplemented. The Windows movie path uses FFmpeg CPU decoding
and GPU upload. Native Windows builds and presentation tests cannot run in this macOS
development environment.
The `windows-build` CI job builds the locked Release binary with the MSVC toolchain,
runs strict Clippy, and checks viewport coordinates and Win32 clipping/input/lifetime
behavior. It does not test DX12 presentation or attached devices.

### GPUI: `Failed to find fxc.exe`

The pinned `gpui-pre-windows` compiles its Windows HLSL shaders with `fxc.exe` in Release builds.
[Microsoft ships FXC with the Windows SDK](https://learn.microsoft.com/en-us/windows/win32/directx-sdk--august-2009-).
If it is absent, use Visual Studio Installer → Modify → Individual components to install
a Windows 10/11 SDK, then reopen the x64 Developer PowerShell.

The [pinned Windows build script](https://docs.rs/crate/gpui-pre-windows/0.3.13/source/build.rs)
checks `GPUI_FXC_PATH`, then `PATH`, then discovers the latest SDK directory from the
Windows registry. It no longer depends on the hardcoded SDK 10.0.26100.0 path used by
GPUI 0.2.2. If automatic discovery still fails, select an installed x64 compiler and
pass its full executable path explicitly:

```powershell
$fxc = Get-ChildItem -Path "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\fxc.exe" -File -ErrorAction SilentlyContinue |
    Sort-Object { [version]$_.Directory.Parent.Name } -Descending |
    Select-Object -First 1

if ($null -eq $fxc) {
    throw "fxc.exe not found. Install a Windows 10/11 SDK through Visual Studio Installer."
}

$env:GPUI_FXC_PATH = $fxc.FullName
Write-Host "Using $env:GPUI_FXC_PATH"
cargo build --release --locked
```

`GPUI_FXC_PATH` must name the executable, not its directory. If an SDK is installed
in a custom location, set this variable to that installation's `fxc.exe` directly.
Use FXC for this pinned GPUI version; its build script invokes FXC-specific arguments
and Shader Model 4.1 profiles. Do not replace it with `dxc.exe`. These settings affect
the current PowerShell process and its Cargo children; no Cargo registry edits or
`cargo clean` are needed to retry the failed build.

A build reporting `x86_64-unknown-linux-gnu` is a Linux/WSL build, with different system
dependencies. On Debian/Ubuntu, a missing `alsa.pc` requires `pkg-config` and
`libasound2-dev`; those packages do not apply to an MSVC build.

## Windows preview and output verification

Normal desktop startup injects the DX12 presentation adapter. Test a procedural sample
first to isolate presentation from external codecs:

```powershell
cargo run --release --locked -- --project sample/03-liquid-contours.vjay.json --output-window
```

The Program preview, enabled node previews, and the separate output window use GPU
textures directly. Select another Output node and choose **Open output** for another
window. A DX12-capable GPU and working driver are required. An old executable that
shows `Native preview unavailable on this platform` uses the former headless Windows
startup path; rebuild the updated source.

Native regression tests create only their own windows and check clipping, mouse hit
testing, shared GPU presentation, DPI changes, and parent-window teardown:

```powershell
cargo test -p vjay-infrastructure --lib desktop::windows::tests -- --ignored --test-threads=1
```

The Windows Rust code and tests were checked from macOS with strict Clippy for
`x86_64-pc-windows-gnu`. That cross-check used a temporary C++ preinclude of
`winsock2.h`, `windows.h`, and `iphlpapi.h`, followed by `#undef interface`, to avoid
an existing Ableton Link / MinGW header macro collision. MSVC remains the documented
Windows build target. Cross-checking does not validate native DX12 display behavior.

`unused_mut`, unused-field warnings, and OpenSSL `LNK4099` messages do not explain
the former unavailable-preview placeholder. LNK4099 means a dependency's debugging
information is unavailable; the linker still produces the executable. See
[Microsoft's LNK4099 reference](https://learn.microsoft.com/en-us/cpp/error-messages/tool-errors/linker-tools-warning-lnk4099?view=msvc-170).
Rendering
failures appear in the application status or as runtime errors after startup.

## Camera access on macOS

The native camera bridge is built from `native/macos/capture.mm` by Cargo using the Apple
SDK. It links system AVFoundation, CoreMedia, CoreVideo, VideoToolbox, CoreGraphics, Metal,
and Metal Performance Shaders for capture and movie playback.
`NSCameraUsageDescription` is embedded in the executable. A local application bundle with
the same declaration can be created after building:

```sh
python3 scripts/bundle_macos.py
open target/release/VJAY.app
```

The bundle is ad-hoc signed for local use, not notarized for distribution. If available,
it includes the locally built Syphon framework. Enable Capture in Media Device Input to
request camera access; the node starts with Capture off. See [capture setup](SETTINGS_AND_CUE.md#media-device-input).

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
machine in this workspace. The DX12 presentation adapter does not validate Spout
interoperability. Use Syphon on macOS.

## Platform scope

| Component | Validation or limitation |
| --- | --- |
| GPUI + wgpu native previews and output windows | Validated on macOS / Apple M2 Pro / Metal |
| Windows native previews and output windows | DX12 adapter implemented; native runtime validation requires a Windows desktop and GPU |
| Media Device Input | macOS AVFoundation / Metal; GPU path tested with generated IOSurfaces; physical devices require local validation |
| VideoToolbox decode and GPU scaling | Validated for supported macOS codecs |
| Linux native presentation; Linux / Windows native GPU-decode PALs | Not implemented |
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

The `gpui-pre 0.3.13` migration was verified on macOS / Apple M2 Pro with formatting,
strict Clippy, 152 regular tests, and five native GPU tests covering graph rendering,
independent outputs, the sample projects, and VideoToolbox playback. The Release app
was also checked with visible text, live node previews, and a separate output window.
The Windows Release CI job is configured; it was not executed in this local macOS run.

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

## Frame trace

Record actual desktop submission intervals and CPU work without synchronous file writes:

```sh
VJAY_FRAME_TRACE=/tmp/vjay-frames.csv cargo run --release -- \
  --project sample/anime-club.vjay.json --output-window
```

Choose your own local media in the patch. The CSV writer flushes every 30 samples and
on orderly shutdown; forced termination may omit the final buffered samples. A saturated
512-record queue drops telemetry rather than blocking rendering. See the architecture
for interpretation: these are CPU submission timings, not display FPS or GPU timestamps.
