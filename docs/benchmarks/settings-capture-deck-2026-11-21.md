# v0.2.0 validation — 2026-11-21

Environment: macOS, Apple M2 Pro, Metal, Rust 1.99.1, GPUI 0.2.2, wgpu 30.
No live-playback FPS benchmark was performed.

## Workspace checks

- `cargo fmt --all --check`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace`: 122 passed; native-dependent tests remain opt-in.
- `cargo build --release`: passed; executable reports `VJAY v0.2.0`.

Cargo reports future-compatibility notices for existing `block` and `proc-macro-error2`
dependencies. These are separate from project Clippy warnings.

## Focused regressions

- Settings survive atomic replacement; malformed and oversized files are rejected.
- Invalid settings or a failed settings write preserve the active configuration.
- GPUI: Cmd+, opens Settings; fields accept edits; Escape, Cancel, Apply, and dropdown
  dismissal preserve the expected draft/saved state. Settings edits do not affect patch nodes.
- By-ear mode ignores BPM values and adjusts speed through its declared node bindings.
- Nudge temporarily changes speed, relative jog starts at the current playhead, pause/resume
  retains the chosen rate, and returning to a stored cue performs a new seek.
- Old ten-parameter `LipSync` projects migrate to BPM ratio mode and retain their cue.
- A partially connected controller is not treated as a complete Speed/Position/Seek route.

## Native checks

Twelve additional opt-in tests passed:

| Area | Tests | Scope |
| --- | ---: | --- |
| Camera GPU bridge | 1 | Generated BGRA IOSurface, native Metal scaling/conversion, first-frame initialization, wgpu sampling, RGBA pixel readback for assertion |
| Core GPU | 1 | Render graph and expected output pixels |
| Samples | 1 | All ten samples, animation, title trigger, both projector outputs |
| Video transport | 2 | Rotation, playback range, pause, and resource reuse |
| Sharing | 2 | Local NDI and Syphon input/output and scaled pixels |
| Audio | 4 | Real CPAL callback at zero output gain, explicit device-rate configuration, invalid-device rejection, volume changes without stream restart, unchanged-position cue seeks, PCM scrubbing and ranges |
| Multiple outputs | 1 | Independent output textures and shared branch execution |

The camera test requests no physical camera or camera permission. Its assertion readback
is test-only; the production capture adapter performs no CPU pixel readback. Physical
camera models, USB capture hardware, camera permission dialogs, long performances, and
multiple physical projectors were not validated in this run.

## Desktop review

A release process used a generated eight-second VideoToolbox H.264 clip, a temporary project,
and `VJAY_SETTINGS_PATH` pointing into `/tmp`. The user's project and settings were not edited.

- Reviewed Inspector, By-ear controls, Media Device Input with capture off, Audio Settings,
  Video Settings, and the frame-rate menu in a 1920 × 1260 editor window.
- Applied 30 Hz through the native UI; the separate JSON settings file contained the change.
- Confirmed the independent Output window was recreated after Apply.
- Verified the macOS bundle's plist, ad-hoc signature, and `v0.2.0` version output.
- Closed the review process after verification.

Screenshots: [Video settings](../images/settings-video.png), [Deck Inspector](../images/deck-sync.png).
The capture teardown drains pending callbacks before releasing the native owner. Inspector
properties follow the node's declared order, with any extension properties appended.
