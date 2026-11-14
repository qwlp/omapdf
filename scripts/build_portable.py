#!/usr/bin/env python3
"""Build one static launcher containing the native reader and its user-space runtime.
Host Wayland compositor, Vulkan ICD/GPU drivers, kernel and desktop portals remain external.
"""
import gzip,hashlib,os,pathlib,platform,struct,subprocess,tomllib
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
            soname=words[0] if '=>' in words else path.name
            files['lib/'+soname]=path.resolve();queue.append(path)
    if source!=binary.resolve():
        # Add dlopen roots under their public SONAME, independent of versioned filenames.
        soname=subprocess.check_output(['readelf','-d',str(source)],text=True)
        import re
        match=re.search(r'\(SONAME\).*\[(.*?)\]',soname)
        if match:files['lib/'+match[1]]=source
files['lib/'+loader]=library(loader).resolve()
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
for package in tomllib.loads((root/'Cargo.lock').read_text()).get('package',[]):
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
# A relative font directory is resolved relative to this fontconfig file.
fonts=b'''<?xml version="1.0"?><!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd"><fontconfig><include ignore_missing="yes">/etc/fonts/fonts.conf</include><dir prefix="relative">fonts</dir><dir>/usr/share/fonts</dir><dir prefix="xdg">fonts</dir><cachedir prefix="xdg">fontconfig</cachedir><alias><family>sans-serif</family><prefer><family>Noto Sans</family></prefer></alias><alias><family>monospace</family><prefer><family>JetBrainsMono NF</family></prefer></alias></fontconfig>'''
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
