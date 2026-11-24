#!/usr/bin/env python3
"""Build one static launcher containing the native reader and its user-space runtime.
Host glibc/ELF loader, GCC runtime, Wayland libraries, Vulkan loader, GPU drivers and portals remain external.
"""
import gzip,hashlib,os,pathlib,platform,struct,subprocess
# Ubuntu 22.04 ships Python 3.10; use its packaged TOML backport.
try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib

root=pathlib.Path(__file__).resolve().parent.parent
output=root/'dist';output.mkdir(exist_ok=True)
binary=root/'target/release/omapdf'
if not binary.exists():raise SystemExit('Build the release reader first')
files={'bin/omapdf':binary,
       'share/icons/hicolor/scalable/apps/io.omapdf.svg':root/'assets/icons/logo.svg',
       'share/icons/hicolor/512x512/apps/io.omapdf.png':root/'assets/icons/logo.png',
       'share/applications/io.omapdf.desktop':root/'assets/io.omapdf.desktop'}
architecture=platform.machine()
platforms={
    'x86_64':('x86_64-unknown-linux-gnu','ld-linux-x86-64.so.2'),
    'aarch64':('aarch64-unknown-linux-gnu','ld-linux-aarch64.so.1'),
}
if architecture not in platforms:raise SystemExit('Portable builds support native Linux x86-64 and ARM64 hosts')
target,loader=platforms[architecture]
# Resolve dlopen roots on Arch and Debian/Ubuntu multiarch systems alike.
cache=subprocess.check_output(['ldconfig','-p'],text=True)
def library(name):
    for line in cache.splitlines():
        words=line.split()
        if words and words[0]==name and '=>' in words:
            candidate=pathlib.Path(words[-1])
            if candidate.is_file():return candidate
    raise SystemExit('Required runtime library not found: '+name)
# GPU ICDs/layers are dlopened into the reader's process. They must share the
# host's loader and glibc (including libm and compatibility DSOs). Bundling only
# an older libc, or mixing a host libc with a bundled loader/libm, is unsafe.
# Mesa also uses Wayland client/protocol symbols; keep the entire Wayland
# library family and Vulkan loader on the host to match its drivers/layers.
# GCC's runtime must also come from the host: Mesa/LLVM may need newer symbol
# versions than the build machine's libgcc_s/libstdc++ provides.
def host_library(name):
    return (name == loader or name in {
        'libc.so.6', 'libm.so.6', 'libmvec.so.1', 'libdl.so.2',
        'libpthread.so.0', 'librt.so.1', 'libutil.so.1', 'libresolv.so.2',
        'libanl.so.1', 'libBrokenLocale.so.1', 'libthread_db.so.1',
        'libgcc_s.so.1', 'libstdc++.so.6',
        'libwayland-client.so.0', 'libwayland-cursor.so.0',
        'libwayland-egl.so.1', 'libwayland-server.so.0', 'libvulkan.so.1',
    } or name.startswith('libnss_'))
queue=[binary]+[library(name) for name in ['libwayland-client.so.0','libwayland-cursor.so.0','libxkbcommon.so.0','libfontconfig.so.1','libfreetype.so.6','libvulkan.so.1']]
seen=set()
while queue:
    source=queue.pop()
    source=source.resolve()
    if source in seen:continue
    seen.add(source)
    report=subprocess.check_output(['ldd',str(source)],text=True)
    for line in report.splitlines():
        words=line.split()
        if 'not found' in line:raise SystemExit(line)
        path=next((pathlib.Path(word) for word in words if word.startswith('/') and pathlib.Path(word).is_file()),None)
        if path:
            # Newer ldd can print an absolute loader path before =>.
            # Normalize it before applying host-library exclusions.
            soname=pathlib.Path(words[0]).name if '=>' in words else path.name
            if not host_library(soname):
                files['lib/'+soname]=path.resolve();queue.append(path)
    if source!=binary.resolve():
        # Add dlopen roots under their public SONAME, independent of versioned filenames.
        soname=subprocess.check_output(['readelf','-d',str(source)],text=True)
        import re
        match=re.search(r'\(SONAME\).*\[(.*?)\]',soname)
        if match and not host_library(match[1]):files['lib/'+match[1]]=source
for path in [root/'LICENSE',root/'THIRD_PARTY.md',root/'assets/AGPL-3.0.txt',root/'assets/icons/LICENSE',root/'assets/fonts/OFL.txt',root/'assets/fonts/Droid-NOTICE',root/'assets/fonts/Noto-OFL.txt']:
    name=path.name if path.parent==root else str(path.relative_to(root)).replace('/','-')
    files['licenses/'+name]=path
for name in ['LICENSE-APACHE','PATCHES.md']:
    files['licenses/gpui/'+name]=root/'vendor/gpui'/name
# Include third-party license notices for bundled native and runtime code.
for package in ['glibc','libxkbcommon','wayland','fontconfig','freetype2','vulkan-icd-loader','expat','zlib','libpng','brotli','bzip2','gcc-libs']:
    directory=pathlib.Path('/usr/share/licenses')/package
    if directory.exists():
        for path in directory.rglob('*'):
            if path.is_file():files['licenses/runtime/'+package+'/'+str(path.relative_to(directory))]=path
registry=pathlib.Path(os.environ.get('CARGO_HOME',str(pathlib.Path.home()/'.cargo')))/'registry/src'
for directory in registry.glob('*/mupdf-sys-0.8.0/mupdf'):
    for path in directory.rglob('*'):
        if path.is_file() and path.name.upper().startswith(('LICENSE','LICENCE','COPYING','OFL','NOTICE')):
            files['licenses/mupdf/'+str(path.relative_to(directory))]=path
# Debian/Ubuntu runtime packages keep their notices under /usr/share/doc.
for source in set(files.values()):
    if str(source).startswith(('/usr/lib/','/lib/')):
        query=subprocess.run(['dpkg-query','-S',str(source)],text=True,capture_output=True) if pathlib.Path('/usr/bin/dpkg-query').exists() else None
        if query and query.returncode!=0 and str(source).startswith('/usr/lib/'):
            query=subprocess.run(['dpkg-query','-S',str(source).replace('/usr/lib/','/lib/',1)],text=True,capture_output=True)
        if query and query.returncode==0:
            package=query.stdout.split(': ',1)[0].splitlines()[0]
            notice=pathlib.Path('/usr/share/doc')/package/'copyright'
            if not notice.exists():notice=pathlib.Path('/usr/share/doc')/package.split(':')[0]/'copyright'
            if notice.is_file():files['licenses/runtime/'+package.replace(':','-')+'/copyright']=notice
# Retain available license/notice texts for every locked Rust dependency,
# including the bundled SQLite migration engine and its Rust bindings.
locked_packages = [package for lock in [root/'Cargo.lock',root/'packaging/launcher/Cargo.lock']
                   for package in tomllib.loads(lock.read_text()).get('package',[])]
for package in locked_packages:
    for directory in registry.glob('*/'+package['name']+'-'+package['version']):
        for path in directory.iterdir():
            if path.name.upper().startswith(('LICENSE','LICENCE','COPYING','NOTICE','COPYRIGHT')):
                if path.is_file():files['licenses/rust/'+directory.name+'/'+path.name]=path
                elif path.is_dir():
                    for notice in path.rglob('*'):
                        if notice.is_file():files['licenses/rust/'+directory.name+'/'+str(notice.relative_to(directory))]=notice
for directory in [pathlib.Path('/usr/share/X11/xkb'),pathlib.Path('/usr/share/X11/locale')]:
    if directory.exists():
        for path in directory.rglob('*'):
            if path.is_file():files['share/X11/'+directory.name+'/'+str(path.relative_to(directory))]=path
files['fonts/ReaderMono.ttf']=root/'assets/fonts/ReaderMono.ttf'
for weight in ['Regular','Medium','Bold']:
    files['fonts/ReaderSans-'+weight+'.ttf']=root/('assets/fonts/ReaderSans-'+weight+'.ttf')
# The host config already supplies system/user directories. Repeating them
# makes fontdb scan every font twice. Keep only the additional bundled directory.
# A relative font directory is resolved relative to this fontconfig file.
fonts=b'''<?xml version="1.0"?><!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd"><fontconfig><include ignore_missing="yes">/etc/fonts/fonts.conf</include><dir prefix="relative">fonts</dir><cachedir prefix="xdg">fontconfig</cachedir><alias><family>sans-serif</family><prefer><family>Noto Sans</family></prefer></alias><alias><family>monospace</family><prefer><family>JetBrainsMono NF</family></prefer></alias></fontconfig>'''
records={name:path.read_bytes() for name,path in files.items()};records['fonts.conf']=fonts
payload=output/'portable-payload.gz'
with gzip.GzipFile(filename=str(payload),mode='wb',mtime=0,compresslevel=9) as archive:
    for name,data in sorted(records.items()):
        encoded=name.encode();archive.write(struct.pack('<I',len(encoded)));archive.write(encoded);archive.write(struct.pack('<Q',len(data)));archive.write(data)
    archive.write(struct.pack('<I',0))
version=hashlib.sha256(payload.read_bytes()).hexdigest()[:20]
env=os.environ.copy();env.update(OMAPDF_PORTABLE_PAYLOAD=str(payload),OMAPDF_PORTABLE_VERSION=version,RUSTFLAGS='-C target-feature=+crt-static')
subprocess.run(['cargo','build','--release','--locked','--offline','--manifest-path',str(root/'packaging/launcher/Cargo.toml'),'--target',target],env=env,check=True)
launcher=root/'packaging/launcher/target'/target/'release/omapdf-portable-launcher'
artifact=output/'omapdf';artifact.write_bytes(launcher.read_bytes());artifact.chmod(0o755)
(output/'runtime-manifest.txt').write_text('\n'.join(sorted(records))+'\n')
print(f'{artifact}: {artifact.stat().st_size/1024**2:.1f} MiB, {len(records)} bundled files')
subprocess.run(['file',str(artifact)],check=True)
