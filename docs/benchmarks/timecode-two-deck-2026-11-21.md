# Timecode readouts and two-deck sample validation — 2026-11-21

Environment: macOS, Apple M2 Pro, Metal, Rust 1.99.1, GPUI 0.2.2, wgpu 30.
No playback FPS benchmark was performed.

## Workspace checks

- `cargo fmt --all --check`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace`: 144 passed; native-dependent tests remain opt-in.
- `cargo build --release`: passed.

The final sample layout also passed focused document and session checks. Cargo's existing
future-compatibility notices for `block` and `proc-macro-error2` remain dependency notices.

## Readouts

- Generic Number controls display time without changing their stored values or ranges.
- Formatting rounds before splitting fields, handles minute/hour carry, negative values,
  hours beyond 24, zero and non-finite values.
- Seek uses observed elapsed seconds and duration. A selected playback subrange keeps its
  normalized interaction range while showing absolute clip time.
- Display settings survive project save/restore and subpatch encapsulation. Legacy Seek
  Control parameter inputs migrate once when the two generic readout inputs are added.
- GPUI tests select format, readout source and optional total using the shared dropdowns.

## Two-deck behavior

The shipped `sample/anime-club.vjay.json` has 45 nodes and 28 assigned controls. It stores
no media paths, assets or external-service configuration. Tests check independent playheads,
timecode readouts, held Nudge, 10 ms Jog, keyboard playback toggling and headphone gains. Visual
crossfade and Blackout do not change those gains or transport states.

An unselected video uses the GPU's empty texture and reports Ready = 0. It opens no decoder.
The sample selects its animated Pattern until Ready becomes 1. Audio compilation skips
unselected sources, allowing one loaded deck to be monitored while the other stays empty.
Errors opening a selected media file remain errors.

## Native checks

Fourteen additional opt-in tests passed:

| Area | Tests |
| --- | ---: |
| Anime-club sample: generated clips, Files loading, one empty deck, VideoToolbox playback, timecode, crossfade, Blackout | 1 |
| All eleven samples: animation, output pixels and title trigger | 1 |
| Core GPU, nested subpatch GPU, Loadable Video and multiple outputs | 4 |
| Video transport and resource reuse | 2 |
| NDI and Syphon loopback | 2 |
| PCM seeking/ranges and silent native audio callback | 4 |

GPU pixel readback is used only for assertions. No new production CPU pixel readback is
introduced by display formatting or the template.

An isolated release app used temporary generated clips with silent audio tracks and zero
monitor gains. The review confirmed both video feeds, moving timecode readouts and visible
file chooser buttons after loading a collection. The user's media, settings and projects
were not changed.

The native checks cover macOS/Metal. Physical MIDI/OSC devices, Windows/Linux presentation,
multiple physical projectors, long performances and automatic beat detection were not
validated. Playback-rate changes still change audio pitch; time stretching is not added.
