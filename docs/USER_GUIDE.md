# User guide

VJAY is a node-based VJ application. Build a visual patch, connect typed ports yourself,
and pin the controls you need for performance. Start with a [sample](SAMPLES.md), or add
nodes to a new project containing an unconnected Output.

## Projects and workspace

Use New / Open / Save / Save As in the toolbar. Shortcuts are Cmd / Ctrl + N / O / S;
Save As is Cmd / Ctrl + Shift + S. New projects start as Untitled and ask for a path on
first save. An asterisk marks unsaved changes. New and Open offer Save / Discard / Cancel
when needed. A failed Open preserves the current project and its location.

Projects store graph wiring, asset references, controls, node positions, panel layout,
and patch zoom. Media files remain external references. Use Save As to keep multiple
sets or variations. A relocated media file can be selected again on its source node.

Drag panel dividers to resize the browser, Inspector, upper panels, and Patch. Perform
expands the control area. Pan with the wheel or middle-button drag. Cmd / Ctrl + wheel
zooms from 25% to 400% around the cursor; Reset view restores 100% and the origin.

## Patch editing

| Action | Operation |
| --- | --- |
| Add a node | Search/filter the browser; choose a result |
| Open the palette | Tab at the pointer; Enter or click to add; Esc/Tab/outside click closes |
| Select | Click; Shift / Cmd / Ctrl + click toggles membership |
| Select a region | Drag empty space; modifier keys add to the selection |
| Select all | Cmd / Ctrl + A |
| Move | Drag a selected header; Esc cancels |
| Delete | Backspace / Delete removes selected nodes, wires, and their control references |
| Duplicate | Alt / Option + header drag, or Cmd / Ctrl + D |
| Copy / paste / cut | Cmd / Ctrl + C / V / X, or the context menu |
| Connect | Drag an output to an input, or an unconnected input to an output |
| Reconnect | Drag a connected input to another input or another output |
| Disconnect | Input context menu, cable menu, or Inspector |
| Insert into a wire | Right-click a cable and choose a compatible node |

Adding a node or importing a file does not create connections. Invalid wiring, cycles,
Esc, or dropping a reconnect on empty space preserves the original wire. Copy/paste keeps
selected internal wires, settings, values, and asset references; external wires and control
assignments are not copied. Text fields retain normal text-editing shortcuts.

Open **Settings** or press **Cmd+,** / **Ctrl+,** to configure audio, video, and new-project defaults.

Ports have distinct types: Image, Value, Audio, Text, and File. Nodes may have multiple
outputs, and parameters expose scalar input ports. Choose the intended output explicitly.

### Parameters and previews

Drag a parameter bar or drag its number vertically. Click a number to type a value.
Shift reduces drag sensitivity to one tenth; Cmd / Ctrl snaps to useful increments.
These modifiers also work on Slider, Knob, Number, and XY pad. Shift-drag on an XY pad
is relative to its current value.

Inspector uses named menus for choices such as Blend mode, Compare, Easing, and Loop.
A parameter driven by a wire is read-only until disconnected. The same choices can still
be driven by scalar signals.

Use the header's preview switch for each image-producing node. Previews show the first
image output and are saved with the project. A disconnected node may render a preview
without joining Program. A failed optional preview reports its own error.

## Control Panel

Add Slider, Knob, Toggle, Button, XY pad, Number, Meter, Label, or Files from **+ Add**.
This also creates the corresponding unconnected control node. Parameter **Pin**, numeric
output **Meter**, text output labels, and **Pin Files to Control Panel** provide other bindings.

Select a control title to edit its name, range, widget, and binding in Inspector. Only the
current binding is shown; **Change target** opens a searchable list. XY pad controls two
adjacent parameters; output meters and labels are read-only.

Drag a title to place a control, drag its corner to resize it, or remove it with ×.
Controls are user-assigned; the software does not require one fixed performance layout.

## Multiple output windows

1. Add one Output node per independent feed and connect its Image input.
2. Select the first Output and press **Open output** in the toolbar or its Inspector.
3. Select another Output and repeat.
4. Move each window to the intended display and use fullscreen as needed.

Opening the same Output again activates its existing window. With no Output selected,
the toolbar opens the Program output. **Use as Program output** selects the editor's
Program view; independent windows keep their own feeds. Shared upstream processing runs
once. See the [Dual Projector sample](SAMPLES.md#two-output-windows).

Missing Program output produces black. An incomplete required branch preserves the last
rendered image and shows an error. Project switching closes old output windows.

## Video and audio

Choose a file on a Video or Image node, or drop a file. Video exposes Video and Audio
outputs; Image supports PNG, JPEG, WebP, BMP, TIFF, and the first GIF frame, including alpha
and EXIF orientation. Route image output into effects or Output yourself.

Audio is optional: connect Audio to Audio Out or Cue / Headphones, directly or through
Audio gain / Audio mix. An unconnected audio output does not open an audio device.
A missing audio track reports an error without stopping video.

Speed, Position, Pause, Loop, Range start/end, and Seek are shared by video and audio.
Loop uses Off/On. Range values are normalized to the whole clip; Position is normalized
within that range. With Loop off, the final frame is held and audio stops. Speed 0 stops
playback. Source and destination ranges must be valid before an edit is applied.

**Start** moves to the range's beginning without changing play/pause state. **End** moves
to its end and pauses. External Position changes can subsequently override manual navigation.
Pause stops the media timeline and shader `iTime`; buttons, envelopes, and LFOs keep running.

macOS uses VideoToolbox decode and GPU scaling. Supported H.264 8-bit 4:2:0, HEVC 8/10-bit
4:2:0, and ProRes 422 play directly. Formats such as H.264 10-bit 4:2:2 are prepared into a
ProRes HQ cache without changing the source. Initial decoding of an unsupported format
uses the CPU; cache encoding uses hardware VideoToolbox. Preparation status is shown and
the media timeline waits for readiness. There is no silent CPU playback fallback.

Cache files use the OS temporary directory under `vjay-video-cache-v1`, source identity,
size, and modification time. The limit is 6 GiB per clip with a 512 MiB free-space reserve.
Unused old caches can be removed before creating new ones. Video input still crosses CPU
memory for RGBA conversion/upload and is not zero-copy.

Explicit seeks and subrange wraps may restart the decoder; ordinary full-range loops
continue it. Audio scrubbing keeps ready PCM while preparing new positions, with an 8 MiB
page cache per source. Speed uses linear resampling and changes pitch. Sample-accurate sync,
pitch-preserving time stretch, and automatic audio matching are not implemented.

## Files and selectable sources

Files selects a file, folder, or UTF-8 M3U/M3U8 playlist and outputs File, Index, Count,
and File name. Filter accepts all / video / image. Connect File to Loadable Video or
Loadable Image. A File output may drive multiple consumers.

Place a selection list on Control Panel through Add → Files or a source's **Pin Files**.
Lists have a scrollable viewport and resizable control cards. Loadable Video/Image can
also select a collection directly when their external File input is unconnected.
An external File takes priority; an empty File clears the previous image.

Folders use filename order without recursion. Playlists retain their order and resolve
relative paths against the playlist. Index starts at 0 and can be driven by MIDI, Counter,
or other signals. Disconnect its signal before choosing manually.

Limits: 4,096 entries, a 4 MiB playlist, 16 cached lists, and 8 workers. Selection changes
load the chosen media through the normal path. Seamless preloaded frame-accurate switching
is not implemented. Original media, playlists, and artwork are read-only.

## Text and artwork

Text accepts a Text body and an optional Font family string. Unconnected inputs use its
settings. A connected empty string displays nothing. Use Text Value for constants, or
connect rekordbox Title / Artist / Album. Text also passes its body through a Text output.

Choose an installed font through the searchable font picker. Font size, Weight, Style,
Alignment, and RGBA have scalar input ports. Styles are Normal / Italic / Oblique;
weights cover 100–900; alignment is Left / Center / Right. Japanese and multiline text
are supported. Missing italic faces fall back to upright glyphs with synthesized slant.

Text images update only when text, format, or extent changes. Use Transform or Composite
for animated positioning and scaling. Connect rekordbox **Album cover** (File) to Loadable
Image, then compose its Image output; missing artwork clears the previous cover.

## Match a clip to the DJ's music

The DJ's floor audio is the reference. VJAY's audio is local monitoring for the VJ.

Use the clip's headphone audio to match the floor track by ear. The media node owns its
Seek, Jog, Pause, Nudge and Cue controls. Deck Rate supplies only Speed. Seek Control
separates the user's Seek request from the actual Playhead feedback shown by its slider.

See [Subpatches and media transport](PATCHING_AND_TRANSPORT.md) for setup, graph-triggered
transport actions, Control Panel seek bars, and named or unnamed patch composition.

### Trigger envelopes

Connect Button **Trigger** → Attack / Release **Gate** → Multiply **A** → Kaleidoscope
**Rotation**. Attack 0.001 seconds, Release 0.5 seconds, and Multiply B = 5 produce a
roughly 5-degree rotation returning to zero. Rotation units are degrees. Button Gate
instead holds the attack while pressed. Zero attack/release makes that direction immediate.

## Keyboard, MIDI, OSC, and Link

- **Keyboard:** set a key name such as `space` or `a`; connect Gate / Trigger. Focused text
  fields keep text input priority.
- **MIDI in:** adding/loading a MIDI node starts reception. Choose All inputs or a device,
  press Learn CC / note, then move a control or play a note. Inspector shows receive counts,
  messages, CC, Note, Velocity, and Gate. Note -1 follows the last note. Channel and mappings
  can also be entered directly. macOS/Linux expose the virtual destination `VJAY MIDI In`.
  Refresh inputs detects new devices; the upper Inputs bar can start/stop MIDI globally.
- **OSC in:** enable reception in the upper Inputs bar. Default bind is `0.0.0.0:9000`;
  OSC address changes the IP/port. Set the node's message address. The first numeric value
  feeds Value / Trigger, including bundles. OSC sending is not implemented.
- **Tap tempo:** use its node action, Tap input, or toolbar Tap to estimate BPM and output
  Beat / Phase / BPM. Keyboard and MIDI can drive Tap.
- **Ableton Link:** Enabled joins a session. Outputs include BPM, Beat, Phase, Pulse, Peers,
  and Playing. Quantum defaults to 4. Editing Tempo sends a tempo request; normal updates
  follow the session. Start/Stop sync is optional; Transport -1 follows, 0 stops, 1 requests
  playback. Link time continues while the local media timeline is paused.

## rekordbox History

The adapter follows the [integration reference](https://nowplayingapp.com/help/reference/integrations/rekordbox/)
and reads DB location/key from `options.json`. `database` and `options_file` default to
`auto`; poll_seconds defaults to 2. Configure paths in Inspector if discovery is insufficient.

Outputs: BPM / Duration / Changed / Ready; Title / Artist / Album / Genre / Label / Key /
File path / Track ID / History time / Status; Album cover (File) / Cover ready.
Changed pulses for a new history row; initial loading displays the latest existing entry.
Artwork resolves content/album ImagePath against the original database location.

This identifies the latest recorded history item, not a guaranteed currently audible track.
It does not read the mixer fader or live playback position, nor change rekordbox history
settings. SQLCipher 3/4 and rekordbox 6-style history are supported; master.pdb and OneLibrary
are not. Keys are not persisted in projects or logs. The original DB/WAL are read only as
files; only a private snapshot is opened by read-only SQLCipher with `query_only`.

## URL video and sharing

**Internal Browser / URL video** resolves a Video URL in a background worker, then plays
its local cache through the normal media pipeline. Connect Image and optionally Audio.
Reload URL retries a failed import. It does not capture arbitrary web pages.

Limits: 2 GiB per video, 8 GiB cache, 8 queued requests, 15 minutes per request, and a
512 MiB free-space reserve. Maximum height accepts 360–2160; available site formats may be
lower. H.264 is preferred for GPU playback. Live streams, login-required videos, and DRM
are unsupported. See [dependency setup](BUILD.md#url-video-imports).

Add NDI, Syphon, or Spout IN/OUT from Video I/O. Connect an OUT's Image and set Send to On.
Empty sender names become `VJAY #ID`. Select an IN source from its searchable list; reopen
the chooser to refresh. Each OUT can use a separate image. An unavailable sender reports
its error without stopping unrelated screen outputs.

NDI uses an external Runtime and bounded asynchronous readback for OUT only. Syphon uses
Metal on macOS. Spout targets Windows; its native build is unverified and Windows desktop
presentation is not implemented. Setup and environment paths are in [BUILD](BUILD.md).

## Node reference

| Category | Nodes |
| --- | --- |
| Sources | Video, Loadable Video, Image, Loadable Image, Text, URL video, Pattern, Solid color, Noise, Checkerboard, Ramp |
| Geometry | Transform, Kaleidoscope, Blur, Pixelate, Edge, RGB split, Crop, Mirror, Displace |
| Color | Color adjustment, Colorize, Invert, Posterize, Levels/Gamma |
| Effects | Luma key, Chroma key, Strobe, GLSL fragment, Threshold, Vignette |
| Compositing | Mix, Blend, Composite/Over, Image selector, Grid, Duplicate grid, Mask, Wipe, Output |
| 3D | 3D Transform, 3D Shape, 3D Tunnel |
| Controls | Seek Control, Files, Text Value, Slider, Knob, Toggle, Button, XY pad, Number |
| Signals | Constant, LFO, Tap tempo, Smooth, Counter, Value selector, Sample and hold, Attack/Release, Signal noise |
| Timing | Deck Rate (By ear / BPM ratio), Deck Sync (legacy) |
| Math | Add, Subtract, Multiply, Divide, Modulo, Power, Min/Max, Absolute, Sine, Cosine, Floor, Round, Fraction, Compare, AND/OR/NOT, Quantize, Map range, Clamp, Easing |
| Organization | Comment, Label, Subpatch, In, Out |
| Input | Keyboard, MIDI in, OSC in |
| Audio | Audio gain, Audio mix, Audio Out, Cue/Headphones |
| Video I/O | NDI IN/OUT, Syphon IN/OUT, Spout IN/OUT |
| External | Ableton Link, rekordbox History |

Composite places Foreground over Background with X/Y, independent scale, rotation, and
opacity. Chroma key creates transparency with RGB key, Tolerance, Softness, and Spill;
Mask uses another image's luminance or alpha. Blend preserves alpha for later compositing.

Image selector switches up to eight images at Index 0–7; Value selector handles numbers.
Grid arranges up to eight inputs left-to-right, top-to-bottom. Duplicate grid repeats one
image up to 16×16 without CPU image copies. Comment/Label are editable annotations.

Neutral settings include Kaleidoscope Segments = 1, Blur Radius = 0, Edge Amount = 0,
Invert Amount = 0, Luma key Threshold = 0, Chroma key Tolerance = 0, Posterize Levels = 256,
and Pixelate Size = 1. Transform defaults are identity. Wipe endpoints select one input.
3D Shape provides Sphere / Cube / Torus / Cylinder with optional image textures;
3D Transform projects a plane, and 3D Tunnel maps an image onto a cylinder. Mesh import
and a general 3D scene graph are not implemented.

Easing maps 0–1 through named curves. Back and Elastic can overshoot; add Clamp if needed.
Sample and hold captures at a trigger edge. Near-zero division/modulo outputs zero;
non-finite results are not forwarded.

### GLSL fragment nodes

Edit shader code and named float uniforms in Inspector. Up to 64 uniforms expose scalar
input ports; each has a name, range, and default. Names must be unique valid identifiers
and cannot shadow reserved built-ins. Renaming updates matching shader identifiers.
Removing an in-use uniform first requires removing its code references; surviving names
retain their wires, values, and control bindings.

```glsl
void mainImage(out vec4 color, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    color = texture(iChannel0, uv);
    color.rgb *= 0.5 + 0.5 * sin(iTime);
}
```

Built-ins are `iTime`, `iResolution`, `iChannel0`, `iChannel1`, and legacy `iParameters`.
Image A/B are optional and default to black. A declared parameter such as
`opacity, 0, 1, 1` is available as `opacity` without a separate declaration.
Changing uniform names/count or code prepares a new pipeline; changing values reuses it.
