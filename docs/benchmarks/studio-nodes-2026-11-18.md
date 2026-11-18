# Visual node validation — 2026-11-18

Environment: macOS, Apple M2 Pro, Metal, Rust 1.99.1, release build.

- Formatting, Clippy for all targets with `-D warnings`, and 86 ordinary tests passed.
- All 22 ignored native tests passed, covering GPU pixels, video, audio, MIDI, Link,
  and read-only rekordbox access.
- New GPU tests checked Composite position/scale/opacity, Selector's eighth input,
  repeated Grid inputs, Chroma key, transparent PNGs, still replacement, and added shaders.
- Tests checked the first, middle, and last of 64 named float uniforms, including
  updates driven by external signals.
- GPUI tests checked Alt-drag/Esc, copy/paste, preview switches, uniform addition,
  browser final-row visibility, and Start/End transport.
- A native GPUI window showed Video/Image previews, Program, and independent output.
  The review project's startup and playback log contained no errors.

A patch using the supplied 4K video, transparent PNG, Composite, GLSL, and Grid rendered
at 1280×720 with five passes and five textures. After warm-up, 180 frames plus final
pixel readback took 407.80 ms; checksum: `aee65b5e3e8b59b0`. This is a headless timing,
not live playback FPS or a performance result for every node configuration.

Native node previews were validated on macOS. Initial decoding/preparation and extra
previews have additional costs. Steady presentation stays on the GPU without CPU pixel
readback. The tested interface allowed eight image inputs per pass, 64 float uniforms,
and a 512 MiB image texture budget.

## Later UI, 3D, and URL-video additions

Formatting, Clippy, the release build, 101 ordinary tests, and 28 native tests passed.
GPU tests covered one-segment Kaleidoscope, effect neutral values, Duplicate Grid, all
3D shapes, and Unicode Text. Minimum 3D scale and maximum camera distance were checked
to ensure the ray-travel bound did not hide valid shapes. GPUI tests covered outside-menu
clicks, menu scrolling, Tab palette, named parameter/property choices, and deletion while
a choice menu was open.

BPM sync tests checked BPM ratios, Trim, held Nudge, phase, and re-seeking the same
position through signal plans and media timing. A short public YouTube video exercised
URL retrieval, cache reuse, and VideoToolbox GPU playback. Downloader cancellation and
asset retention after deleting its source node were also checked.

A patch containing 3D Shape + Text + Blend, Checkerboard + Duplicate Grid, and Noise used
six passes and four textures at 960×540. After warm-up, 120 frames plus final readback
took 44.14 ms; checksum: `cd2d5465e52d59d0`. This is also not realtime playback FPS.
A native GPUI window showed Japanese text, 3D compositing, previews, the upper Inputs bar,
Inspector sections, and named modes without logged errors.
