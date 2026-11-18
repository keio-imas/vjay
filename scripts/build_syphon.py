#!/usr/bin/env python3
"""Build pinned BSD-licensed Syphon with Apple Command Line Tools, without installing it."""
from pathlib import Path
import concurrent.futures, io, json, plistlib, shutil, subprocess, urllib.request, zipfile
ROOT = Path(__file__).resolve().parent.parent
COMMIT = 'f4761677a45b8034a3c2069ec0f3d2553da81fba'
SOURCE = ROOT / '.dependencies' / ('Syphon-Framework-' + COMMIT)
if not SOURCE.exists():
    data = urllib.request.urlopen('https://codeload.github.com/Syphon/Syphon-Framework/zip/' + COMMIT).read()
    with zipfile.ZipFile(io.BytesIO(data)) as archive:
        archive.extractall(ROOT / '.dependencies')
BUILD = ROOT / 'target' / 'syphon-build'
FRAMEWORK = ROOT / 'target' / 'syphon' / 'Syphon.framework'
VERSION = FRAMEWORK / 'Versions' / 'A'
RESOURCES = VERSION / 'Resources'
BUILD.mkdir(parents=True, exist_ok=True)
RESOURCES.mkdir(parents=True, exist_ok=True)
INCLUDE=BUILD / "include"
INCLUDE.mkdir(exist_ok=True)
if not (INCLUDE / "Syphon").exists(): (INCLUDE / "Syphon").symlink_to(SOURCE)
# CLT has the runtime Metal compiler, but no offline metallib tool. Compile the
# upstream shader once at sender creation; rendering and publishing remain GPU-only.
source = (SOURCE / 'SyphonServerRendererMetal.m').read_text()
old = '[device newDefaultLibraryWithBundle:bundle error:&error]'
new = '[device newLibraryWithSource:[NSString stringWithContentsOfFile:[bundle pathForResource:@"SyphonMetalShaders" ofType:@"metal"] encoding:NSUTF8StringEncoding error:&error] options:nil error:&error]'
assert old in source
(BUILD / 'SyphonServerRendererMetal.m').write_text(source.replace(old, new))
shader = (SOURCE / 'SyphonMetalShaders.metal').read_text().replace('#include "SyphonServerMetalTypes.h"', (SOURCE / 'SyphonServerMetalTypes.h').read_text())
(RESOURCES / 'SyphonMetalShaders.metal').write_text(shader)
shutil.copy2(SOURCE / 'License.txt', RESOURCES / 'License.txt')
with (RESOURCES / 'Info.plist').open('wb') as file:
    plistlib.dump(dict(CFBundleExecutable='Syphon', CFBundleIdentifier='info.v002.Syphon', CFBundlePackageType='FMWK', CFBundleVersion='3.0', CFBundleName='Syphon'), file)
files = list(SOURCE.glob('*.m')) + list(SOURCE.glob('*.c'))
def compile_file(file):
    src = BUILD / file.name if file.name == 'SyphonServerRendererMetal.m' else file
    obj = BUILD / (file.stem + '.o')
    args = ['clang', '-c', '-O2', '-fblocks', '-DGL_SILENCE_DEPRECATION', '-Wno-deprecated-declarations', '-mmacosx-version-min=12.0', '-I', str(SOURCE), '-I', str(INCLUDE), '-include', str(SOURCE / 'Syphon_Prefix.pch')]
    if file.suffix == '.m': args.append('-fobjc-arc')
    subprocess.run(args + [str(src), '-o', str(obj)], check=True)
    return str(obj)
with concurrent.futures.ThreadPoolExecutor(max_workers=4) as workers:
    objects = list(workers.map(compile_file, files))
subprocess.run(['clang', '-dynamiclib', '-mmacosx-version-min=12.0', '-install_name', '@rpath/Syphon.framework/Versions/A/Syphon', *objects, '-framework', 'Cocoa', '-framework', 'Metal', '-framework', 'IOSurface', '-framework', 'OpenGL', '-o', str(VERSION / 'Syphon')], check=True)
for link, target in [(FRAMEWORK / 'Versions' / 'Current', 'A'), (FRAMEWORK / 'Syphon', 'Versions/Current/Syphon'), (FRAMEWORK / 'Resources', 'Versions/Current/Resources')]:
    if not link.exists(): link.symlink_to(target)
print(FRAMEWORK)
