# Asynchronous pipeline and copy elimination — 2026-11-22

VJAY v0.3.1 development tree; Apple M2 Pro / Metal, macOS 26.4.1, Rust 1.99.1,
FFmpeg 8.1.2. Baseline commit: `5d306a8260e4d9532f9e4a99b6a8c669f0061626`.
Both versions already use native VideoToolbox decoding and Metal frame transfer.
This comparison is separate from the earlier CPU-RGBA versus native comparison.

## Changes

- Issue every video's decode request before waiting for any one source. This overlaps
  independent I/O and hardware decoding instead of serializing source A then source B.
- Batch native conversions in one Metal command buffer and compute encoder per frame.
  Always end/commit the batch on source errors so already registered GPU waits are signalled.
- Forward compatible external source textures through an open kernel capability. Dedicated
  slots keep persistent inputs out of the transient pool, avoiding both copy passes and
  overwritten input frames. Captures and video-sharing outputs retain COPY_SRC support.
- Decode/resize file images and rasterize generated images on two workers, with sixteen
  queued jobs and two queued results. Each input coalesces changes behind one in-flight job.
  Generated revisions make progress under rapid edits; superseded file results are discarded.
  A four-entry/64 MiB weighted LRU shares immutable CPU raster buffers. Last completed GPU
  images remain visible during replacement, and source textures persist across updates.
- Execute project parsing, serialization, writes and synchronization on GPUI's background
  executor. Application epochs/revisions keep late completions from replacing new projects
  or clearing edits made during a save. Save-before-close does not discard newer edits.
- Editor/node-preview swapchains do not each wait for vsync; independent output windows
  preserve the configured vsync setting. Add opt-in asynchronous CPU frame telemetry.

## Headless comparison

Two locally generated 3840×2160, 60 fps, four-second H.264 4:2:0 fixtures at 24 Mbit/s,
with distinct hue settings, feed `sample/anime-club.vjay.json`. Node previews and audio
are disabled; no output window is created. Both executables warm five frames at time zero.
Three runs alternate before/after executable order at each render extent. Compilation and
other GPU tests are idle throughout measurements. The baseline executable was copied before
editing; only the benchmark's appended renderer counters are absent from its CSV rows.

Workloads and fenced timing match the [previous method](playback-2026-11-22.md#measurement-method):
120 sequential requests at i/60 seconds; 120 nearby repeated jogs across a small frame set;
16 seeks cycling eight widely separated positions. Every request waits for GPU completion
without pixel readback. These are fenced end-to-end latencies, not GPU timestamps or
presentation FPS. Cold seeks still need reference decoding; nearby cache hits do not
represent arbitrary seek performance.

Raw data: [CSV](async-pipeline-2026-11-22.csv). Summary values are the median of the three
within-run percentiles; tail statistics from the 16-seek workload have limited resolution.

| Extent | Workload | Before p50 / p95 / p99 (ms) | After p50 / p95 / p99 (ms) |
| --- | --- | ---: | ---: |
| 1920×1080 | sequential | 8.44 / 11.60 / 13.38 | 6.82 / 10.31 / 11.37 |
| 1920×1080 | jog | 1.81 / 2.82 / 4.33 | 2.40 / 3.60 / 4.07 |
| 1920×1080 | seek | 36.22 / 210.24 / 210.24 | 24.30 / 202.09 / 202.09 |
| 3840×2160 | sequential | 11.95 / 17.30 / 17.81 | 9.51 / 13.61 / 15.40 |
| 3840×2160 | jog | 4.40 / 6.23 / 7.51 | 3.16 / 5.76 / 6.40 |
| 3840×2160 | seek | 37.35 / 222.28 / 222.28 | 26.51 / 207.41 / 207.41 |

The two video-copy passes are eliminated: the current counters report seven encoded
image passes and two forwarded sources. Pooled 4K RGBA textures occupy 132,710,400 bytes
(four full-extent images), excluding external sources, native decoder surfaces, output
swapchains and driver memory. The old six-texture pool occupied 199,065,600 bytes.
This reduces pooled storage by 66,355,200 bytes (63.28 MiB), not total process memory.
Native movie delivery remains zero CPU pixel bytes in every measured workload.

The 1080p nearby-jog p95 did not improve in this run (2.82 to 3.60 ms). The 4K sequential
p95 improved from 17.30 to 13.61 ms, but cold-seek p95 remained about 207 ms; decoding a
long GOP is still required. No universal speedup or sustained display FPS is claimed.

## Desktop trace

The Release GPUI app ran the loaded two-deck patch for approximately 15 seconds, with
node previews enabled and one independent output window at a 1920×1080 render extent.
Both inputs were the same generated 4K60 fixtures. Existing user preferences requested
120 Hz (8.333 ms); they were read without modification. After excluding two seconds
of startup, 1115 samples remained. All 1290 captured submissions were successful
and their counters were contiguous. The fixtures contain no audio tracks.

| CPU measure | p50 (ms) | p95 (ms) | p99 (ms) | Maximum (ms) |
| --- | ---: | ---: | ---: | ---: |
| interval_ms | 8.469 | 17.553 | 19.866 | 65.185 |
| control_ms | 0.010 | 0.016 | 0.023 | 0.036 |
| render_ms | 2.462 | 7.183 | 8.330 | 14.844 |
| total_ms | 2.471 | 7.202 | 8.344 | 14.850 |

Raw trace: [CSV](desktop-submit-2026-11-22.csv). Submission intervals still miss the
requested 120 Hz cadence, with a roughly 65 ms maximum gap in this short run. Renderer
CPU time alone does not explain every gap: UI/executor scheduling remains shared.
This is a validation trace, not a sustained display-FPS result or proof of smooth 120 Hz output.

`VJAY_FRAME_TRACE` records actual desktop CPU submission intervals and separates renderer
work from control/preparation time. Formatting and file writes use a separate bounded I/O
worker. It does not measure display scanout, GPU completion or newly displayed video frames.
No before/after desktop trace exists for the uninstrumented baseline executable.

## Correctness and validation

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` and
  `cargo test --workspace` passed (152 regular tests).
- Twenty-seven selected native GPU tests passed, including decoder fanout/copy elimination,
  nonblocking image replacement, saturated exact queues and rapidly edited generated images.
- Extension shaders, generated-IOSurface camera conversion, NDI loopback and Syphon loopback
  passed separately (31 native GPU tests total). Physical camera and Windows Spout were not tested.
- Tests cover discarded stale image results, bounded queues, nonblocking worker teardown,
  raster-cache ownership, edits during save, stale project completions and failed writes.

The frame loop still runs on GPUI's thread. Background image/file work and preview changes
remove specific stalls; they do not provide a separate realtime render thread or eliminate
shader compilation and all edit-time preparation pauses. Native movie input remains macOS-only
and 8-bit SDR. Long thermal runs, eight concurrent sources and multiple physical 4K outputs
are outside these measurements.

## Reproduction

Generate fixtures with the [earlier commands](playback-2026-11-22.md#reproduction). Save a
baseline `playback_bench` executable before changing the implementation, build the new
Release example, then run them alternately while compilation is idle:

```sh
cargo build --release --example playback_bench
/path/to/playback-before /tmp/vjay-a.mp4 /tmp/vjay-b.mp4 3840x2160 120 native
target/release/examples/playback_bench /tmp/vjay-a.mp4 /tmp/vjay-b.mp4 3840x2160 120 native
```

Repeat three times at each extent. See [frame tracing](../BUILD.md#frame-trace) for desktop
submission diagnostics. The native asynchronous-image and schedule tests can be run with:

```sh
cargo test -p vjay-infrastructure --test image_jobs --test video_schedule \
  -- --ignored --test-threads=1
```

## Checkpoint

Optimization was paused at the user's request after final verification. The Release app
and example were rebuilt, the macOS bundle was ad-hoc signed and verified, and v0.3.1
was retained. Remaining main-thread isolation and cold-seek work are not included here.
