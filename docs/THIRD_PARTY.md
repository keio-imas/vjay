# Third-party dependencies and licenses

- The GPUI frontend uses Apache-2.0-licensed `gpui-pre` and `gpui-pre-platform`,
  pinned to `0.3.13` (upstream Zed revision
  `125ca8672e928b19a62c2e4b761fb29fe2a34ae3`). Longbridge publishes these snapshots
  of Zed's GPUI; upstream license and notice files remain applicable. See the
  [published package](https://docs.rs/crate/gpui-pre/0.3.13) and
  [upstream license](https://github.com/zed-industries/zed/blob/main/crates/gpui/LICENSE-APACHE).
- `rusty_link 0.4.9` embeds the Ableton Link SDK. The binding and SDK are offered under
  GPL-2.0-or-later, with a commercial license option from Ableton. The project source's
  `license = "MIT"` does not replace these terms for a linked binary. A proprietary
  distribution needs the appropriate Ableton license. See the
  [binding license notice](https://github.com/anzbert/rusty_link#license) and
  [Ableton Link licensing](https://github.com/Ableton/link#license).
- `rusqlite` is MIT licensed. Its `bundled-sqlcipher-vendored-openssl` dependency bundles
  SQLCipher, SQLite, and OpenSSL; their upstream notices apply. See
  [SQLCipher license](https://github.com/sqlcipher/sqlcipher/blob/master/LICENSE.md),
  [SQLite copyright](https://www.sqlite.org/copyright.html), and
  [OpenSSL license](https://www.openssl.org/source/license.html).
- `blowfish` and `base64` are MIT OR Apache-2.0. The rekordbox options decoder uses the
  documented ECB format, including the legacy unpadded 64-byte key. Source reference:
  [rekordbox-connect](https://github.com/chrisle/rekordbox-connect).
- FFmpeg and FFprobe are external executables selected at runtime. Their license and
  available codecs depend on the installed build. They are not bundled by this project.
  See [FFmpeg legal information](https://ffmpeg.org/legal.html).
- URL imports invoke external `yt-dlp`; the project venv is excluded from version
  control. Its core uses the Unlicense, while EJS and optional dependencies carry
  their own notices. See [yt-dlp dependencies and licenses](https://github.com/yt-dlp/yt-dlp#dependencies).
- Text shaping and rasterization use `cosmic-text` and its font dependencies.
  Their upstream licenses and the licenses of installed fonts apply. See
  [cosmic-text](https://github.com/pop-os/cosmic-text).
- NDI Runtime is an external dynamically loaded library. It is not bundled or redistributed by
  this repository. Its installation and redistribution terms are defined by NDI; see the
  [NDI SDK documentation](https://docs.ndi.video/all/developing-with-ndi/sdk).
  The standard async video-send API retains frame memory until a later send or synchronization;
  the adapter preserves that lifetime and uses bounded asynchronous readback only for NDI OUT.
- macOS Syphon uses the upstream BSD-licensed
  [Syphon Framework](https://github.com/Syphon/Syphon-Framework), pinned to
  `f4761677a45b8034a3c2069ec0f3d2553da81fba` by `scripts/build_syphon.py`.
  Sources are downloaded to `.dependencies/`, and the built framework retains `License.txt`.
  The script builds a local copy and adapts its Metal shader loading for Command Line Tools;
  it does not install anything into system locations.
- The Windows Spout bridge builds the BSD-licensed
  [Spout2 SDK](https://github.com/leadedge/Spout2), pinned to
  `6b68a5b4fdeca6130b851e5f9366467a844860e5` in `native/spout/CMakeLists.txt`.
  Keep the upstream SDK license when distributing the bridge. Windows build and interoperability
  have not been validated in this macOS workspace.

VJAY project source is licensed under the [MIT license](../LICENSE).
This list covers the native integrations. Cargo.lock records the complete
Rust dependency versions; upstream license files remain authoritative.
