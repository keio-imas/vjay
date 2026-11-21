# Subpatch and media transport validation — 2026-11-21

Environment: macOS, Apple M2 Pro, Metal, Rust 1.99.1, GPUI 0.2.2, wgpu 30.
No live-playback FPS benchmark was performed.

## Workspace checks

- `cargo fmt --all --check`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace`: 138 passed. Native-dependent tests remain opt-in.
- `cargo build --release`: passed.

Existing `block` and `proc-macro-error2` dependencies produce Cargo future-compatibility
notices, separate from project Clippy warnings.

## Transport regressions

- A media node owns its playhead, local Pause state, Cue marker and held Nudge state.
  Inspector actions do not create a rate node, change a peer node's parameters or prepare
  GPU resources. A separate connected Speed source retains its configured rate.
- Button and Keyboard signals control Jog, Pause/Play, Cue and held Nudge through declared
  media inputs. Releasing Nudge restores the base rate.
- Seek Control displays actual playback feedback without repeatedly seeking to the last
  request. An edited request moves playback. Feedback connections also work through
  subpatch boundaries; ordinary instantaneous cycles remain invalid.
  Repeating the same requested position emits a new seek without changing its numeric value.
- Saving immediately after Set cue preserves the marker. Non-looping playback publishes
  its stopped state and can restart from the beginning of its selected range.
- Legacy Video and Loadable Video ports migrate without changing their parameter indices.
  A legacy Deck Sync Cue is preserved without automatically rewiring the graph.

## Subpatch regressions

- Encapsulation retains internal wiring, crossing connections, settings, control bindings,
  assets and nested definitions. Typed boundaries support Image, Value, Audio, Text and File.
- Nested expansion produces the same GPU pixels and processing-pass count as the original
  graph. Editing a child changes the root output; saving and restoring preserves it.
- Name and order changes preserve compatible wires by persistent boundary identity.
- Both new connections and cable reconnections validate the expanded graph. An invalid
  instantaneous cycle preserves the original wire, including when a container also
  publishes delayed playback feedback.
- Copying instances creates independent node identities. Copying a boundary creates a new
  port identity; named instances can be loaded more than once without sharing runtime state.
  Inserting a node into a wire also reserves identities used by nested children.
- Project Save inside an open child saves the complete root project. Named patches save
  standalone documents; reload is explicit. Invalid reloads leave the instance intact.
- GPUI tests cover selection encapsulation, opening a child, returning through breadcrumbs,
  transport without a controller and release of held Nudge.

## Native checks

Thirteen additional opt-in tests passed:

| Area | Tests | Scope |
| --- | ---: | --- |
| Core GPU | 1 | Expected output pixels |
| Subpatch GPU | 1 | Exact output pixels before/after nested encapsulation, unchanged processing-pass count, child edits, save/restore |
| Loadable Video | 1 | Folder selection and signal-controlled switching with hardware decoding |
| Samples | 1 | All ten samples, animation, title trigger and both projector outputs |
| Video transport | 2 | Rotation, playback range, Pause and topology resource reuse |
| Multiple outputs | 1 | Independent textures and shared branch execution |
| Sharing | 2 | Local NDI and Syphon input/output loopback |
| Audio | 4 | Real CPAL callbacks at zero gain, local Pause/Nudge, decoded PCM seeks, ranges and cache reuse |

Pixel readback in GPU assertions is test-only. Production rendering does not add a CPU
readback for subpatches. The previously authorized NDI OUT adapter remains an exception.

## Desktop review and limits

An isolated release process used a generated eight-second VideoToolbox H.264 clip, a
temporary project and a separate settings path. The user's project and settings were not
edited. The review confirmed the actual Playhead slider, a Button wired to Toggle play,
the container preview and child In/Out editing while Program continued rendering. The
review process was closed afterward.

Screenshots: [root patch and feedback](../images/transport-feedback.png),
[subpatch editor](../images/subpatch-editor.png).

This run did not validate physical MIDI controllers, network OSC senders, Windows Spout,
multiple physical projectors, long performances, automatic beat detection or time stretching.
Pitch changes with playback rate. Subpatch graph bounds are documented in the
[transport and subpatch guide](../PATCHING_AND_TRANSPORT.md#execution-and-limits).
