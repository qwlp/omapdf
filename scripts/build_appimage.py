#!/usr/bin/env python3
"""Wrap the native portable build in an AppImage using official appimagetool."""
import argparse
import os
from pathlib import Path
import platform
import shutil
import subprocess
import tempfile

# Ubuntu 22.04 ships Python 3.10; use its packaged TOML backport.
try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib

parser = argparse.ArgumentParser()
parser.add_argument('--appimagetool', required=True, type=Path)
args = parser.parse_args()
root = Path(__file__).resolve().parent.parent
architecture = platform.machine()
if architecture not in {'x86_64', 'aarch64'}:
    raise SystemExit('AppImage builds support native Linux x86-64 and ARM64 hosts')
portable = root / 'dist/omapdf'
if not portable.is_file():
    raise SystemExit('Build the release reader and portable executable first')
version = tomllib.loads((root / 'Cargo.toml').read_text())['package']['version']
output = root / 'dist' / f'Omapdf-{version}-{architecture}.AppImage'
with tempfile.TemporaryDirectory(prefix='omapdf-appimage-') as temporary:
    appdir = Path(temporary) / 'Omapdf.AppDir'
    binary = appdir / 'usr/bin/omapdf'
    binary.parent.mkdir(parents=True)
    shutil.copy2(portable, binary)
    binary.chmod(0o755)
    shutil.copy2(root / 'assets/io.omapdf.desktop', appdir / 'io.omapdf.desktop')
    shutil.copy2(root / 'assets/icons/logo.svg', appdir / 'io.omapdf.svg')
    (appdir / '.DirIcon').symlink_to('io.omapdf.svg')
    launcher = appdir / 'AppRun'
    launcher.write_text('#!/bin/sh\nexec "$APPDIR/usr/bin/omapdf" "$@"\n')
    launcher.chmod(0o755)
    notices = appdir / 'usr/share/licenses/omapdf'
    notices.mkdir(parents=True)
    for name in ['LICENSE', 'THIRD_PARTY.md']:
        shutil.copy2(root / name, notices / name)
    shutil.copy2(root / 'assets/APPIMAGE-RUNTIME-LICENSE', notices / 'APPIMAGE-RUNTIME-LICENSE')
    environment = dict(os.environ, ARCH=architecture, APPIMAGE_EXTRACT_AND_RUN='1', VERSION=version)
    subprocess.run([str(args.appimagetool.resolve()), '--no-appstream', str(appdir), str(output)], env=environment, check=True)
output.chmod(0o755)
print(output)
