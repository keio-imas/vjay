#!/usr/bin/env python3
"""Package the release binary with the camera permission declaration."""
import pathlib
import shutil
import subprocess
import sys

root = pathlib.Path(__file__).resolve().parents[1]
if sys.platform != "darwin":
    raise SystemExit("This bundle target requires macOS")
binary = root / "target/release/vjay"
if not binary.is_file():
    raise SystemExit("Run cargo build --release first")
contents = root / "target/release/VJAY.app/Contents"
(contents / "MacOS").mkdir(parents=True, exist_ok=True)
shutil.copy2(binary, contents / "MacOS/vjay")
shutil.copy2(root / "native/macos/Info.plist", contents / "Info.plist")
framework = root / "target/syphon/Syphon.framework"
if framework.is_dir():
    destination = contents / "Frameworks/Syphon.framework"
    if destination.exists():
        shutil.rmtree(destination)
    shutil.copytree(framework, destination, symlinks=True)
subprocess.run(["codesign", "--force", "--deep", "--sign", "-", str(contents.parent)], check=True)
print(contents.parent)
