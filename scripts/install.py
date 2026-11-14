#!/usr/bin/env python3
"""Install Omapdf's portable artifact and desktop assets with atomic replacement."""
import argparse
import datetime
import os
from pathlib import Path
import shutil
import tempfile

parser = argparse.ArgumentParser()
parser.add_argument('--prefix', default='/usr/local')
parser.add_argument('--destdir', default='')
args = parser.parse_args()
root = Path(__file__).resolve().parent.parent
prefix = Path(args.destdir) / args.prefix.lstrip('/') if args.destdir else Path(args.prefix)
stamp = datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%S%fZ')


def install(source, relative, mode=0o644, data=None):
    destination = prefix / relative
    destination.parent.mkdir(parents=True, exist_ok=True)
    if destination.exists() or destination.is_symlink():
        backup = destination.with_name(destination.name + '.backup-' + stamp)
        shutil.copy2(destination, backup, follow_symlinks=True)
    fd, staging = tempfile.mkstemp(prefix='.' + destination.name + '-', dir=destination.parent)
    try:
        with os.fdopen(fd, 'wb') as output:
            output.write(source.read_bytes() if data is None else data)
            output.flush()
            os.fsync(output.fileno())
            os.fchmod(output.fileno(), mode)
        os.replace(staging, destination)
    finally:
        if os.path.exists(staging):
            os.unlink(staging)


install(root / 'dist/omapdf', 'bin/omapdf', 0o755)
desktop = (root / 'assets/io.omapdf.desktop').read_text()
executable = str(Path(args.prefix) / 'bin/omapdf')
desktop = desktop.replace('Exec=omapdf %f', f'Exec={executable} %f')
desktop = desktop.replace('TryExec=omapdf', f'TryExec={executable}')
install(root / 'assets/io.omapdf.desktop', 'share/applications/io.omapdf.desktop', data=desktop.encode())
install(root / 'assets/icons/logo.svg', 'share/icons/hicolor/scalable/apps/io.omapdf.svg')
install(root / 'assets/icons/logo.png', 'share/icons/hicolor/512x512/apps/io.omapdf.png')
install(root / 'assets/prefs.config', 'share/omapdf/prefs.config')
install(root / 'assets/keys.config', 'share/omapdf/keys.config')
for source, name in [
    ('LICENSE', 'LICENSE'),
    ('THIRD_PARTY.md', 'THIRD_PARTY.md'),
    ('assets/icons/LICENSE', 'LUCIDE-LICENSE'),
    ('assets/AGPL-3.0.txt', 'MUPDF-AGPL-3.0'),
    ('assets/fonts/OFL.txt', 'FONT-OFL'),
    ('assets/fonts/Noto-OFL.txt', 'NOTO-OFL'),
    ('assets/fonts/Droid-NOTICE', 'DROID-NOTICE'),
]:
    install(root / source, 'share/licenses/omapdf/' + name)
print(f'Installed Omapdf under {prefix}')
