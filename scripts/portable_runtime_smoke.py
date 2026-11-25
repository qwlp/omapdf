#!/usr/bin/env python3
"""Check real portable library resolution; --gpu also enumerates host Vulkan GPUs.

Requires a built dist/omapdf, a C compiler for --gpu.
All extraction and probe files are isolated in a temporary directory.
"""
import argparse
import os
import pathlib
import platform
import re
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--binary', type=pathlib.Path, default=ROOT / 'dist/omapdf')
parser.add_argument('--gpu', action='store_true')
parser.add_argument('--software-renderer', action='store_true', help='Expect the separate AppImage CPU runtime')
parser.add_argument('--max-glibc', help='Maximum permitted GLIBC symbol version (release baseline: 2.35)')
args = parser.parse_args()
loader = {
    'x86_64': '/lib64/ld-linux-x86-64.so.2',
    'aarch64': '/lib/ld-linux-aarch64.so.1',
}[platform.machine()]
with tempfile.TemporaryDirectory(prefix='omapdf-runtime-test-') as temporary:
    temporary = pathlib.Path(temporary)
    env = dict(os.environ, XDG_CACHE_HOME=str(temporary), LD_DEBUG='libs,versions')
    result = subprocess.run([str(args.binary.resolve()), '--version'], env=env,
                            text=True, capture_output=True, check=True, timeout=60)
    runtime, = (temporary / 'omapdf/runtime').glob('*/COMPLETE')
    runtime = runtime.parent
    # Inspect the extracted artifact, not just a build-time manifest. Check all
    # glibc companions too: deleting only libc still allows an ABI mismatch.
    for path in (runtime / 'lib').rglob('*'):
        if path.name == 'libxcb-xkb.so.1':continue
        assert not path.name.startswith(('ld-linux', 'libnss_', 'libxcb', 'libX', 'libbsd.so.', 'libmd.so.')), path
        assert path.name not in {
            'libc.so.6', 'libm.so.6', 'libmvec.so.1', 'libdl.so.2',
            'libpthread.so.0', 'librt.so.1', 'libutil.so.1', 'libresolv.so.2',
            'libanl.so.1', 'libBrokenLocale.so.1', 'libthread_db.so.1',
            'libgcc_s.so.1', 'libstdc++.so.6',
            'libwayland-client.so.0', 'libwayland-cursor.so.0',
            'libwayland-egl.so.1', 'libwayland-server.so.0', 'libvulkan.so.1',
        }, path
    software = runtime / 'software-vulkan/lib'
    if args.software_renderer:
        assert (software / 'libvulkan_lvp.so').is_file(), 'CPU renderer is missing'
        for path in software.iterdir():
            assert not path.name.startswith(('ld-linux', 'libnss_')), path
            assert path.name not in {
                'libc.so.6', 'libm.so.6', 'libmvec.so.1', 'libdl.so.2',
                'libpthread.so.0', 'librt.so.1', 'libutil.so.1', 'libresolv.so.2',
                'libanl.so.1', 'libBrokenLocale.so.1', 'libthread_db.so.1',
                'libgcc_s.so.1', 'libstdc++.so.6',
            }, path
    else:
        assert not software.exists(), 'Standalone executable unexpectedly bundles a CPU renderer'
    if args.max_glibc:
        baseline = tuple(map(int, args.max_glibc.split('.')))
        # The static outer launcher hides the actual ABI requirements from
        # AppImage scanners. Inspect every ELF in the extracted payload.
        checked = 0
        for path in runtime.rglob('*'):
            if not path.is_file():
                continue
            with path.open('rb') as source:
                if source.read(4) != b'\x7fELF':
                    continue
            report = subprocess.check_output(['readelf', '--version-info', str(path)], text=True)
            versions = {tuple(map(int, version.split('.')))
                        for version in re.findall(r'\bGLIBC_(\d+(?:\.\d+)+)\b', report)}
            too_new = sorted(version for version in versions if version > baseline)
            assert not too_new, f'{path}: GLIBC requirements {too_new} exceed {args.max_glibc}'
            assert 'GLIBC_PRIVATE' not in report, f'{path}: private glibc ABI dependency'
            checked += 1
        assert checked, 'No embedded ELF files found'
        print(f'{checked} embedded ELF files require at most GLIBC_{args.max_glibc}')
    # LD_DEBUG records the files actually initialized, unlike search attempts.
    initialized = [line.split('calling init:', 1)[1].strip()
                   for line in result.stderr.splitlines() if 'calling init:' in line]
    for soname in ('libc.so.6', 'libm.so.6', 'libgcc_s.so.1'):
        loaded = [path for path in initialized if pathlib.Path(path).name == soname]
        assert loaded, (soname, result.stderr)
        assert all(str(runtime) not in path for path in loaded), loaded
    for soname in ('libxkbcommon-x11.so.0', 'libxcb-xkb.so.1'):
        loaded = [path for path in initialized if pathlib.Path(path).name == soname]
        assert loaded and all(str(runtime) in path for path in loaded), (soname, loaded)
    assert 'version lookup error' not in result.stderr, result.stderr
    print('Portable reader uses host libc, libm and libgcc; hardware path uses host graphics libraries; bundled keyboard bridge; CPU runtime checked separately')
    if args.gpu:
        source = temporary / 'vulkan-probe.c'
        source.write_text(r'''
#include <dlfcn.h>
#include <stdio.h>
#include <stdint.h>
// Minimal Vulkan 1.0 ABI declarations keep this runtime probe independent of
// development headers. No application/device features are requested.
typedef void *VkInstance;
typedef int32_t VkResult;
typedef struct {
    uint32_t sType;
    const void *pNext;
    uint32_t flags;
    const void *pApplicationInfo;
    uint32_t enabledLayerCount;
    const char *const *ppEnabledLayerNames;
    uint32_t enabledExtensionCount;
    const char *const *ppEnabledExtensionNames;
} VkInstanceCreateInfo;
typedef VkResult (*PFN_vkCreateInstance)(const VkInstanceCreateInfo *, const void *, VkInstance *);
typedef VkResult (*PFN_vkEnumeratePhysicalDevices)(VkInstance, uint32_t *, void *);
typedef void (*PFN_vkDestroyInstance)(VkInstance, const void *);
#define VK_SUCCESS 0
#define VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO 1
int main(void) {
    void *library = dlopen("libvulkan.so.1", RTLD_NOW | RTLD_LOCAL);
    if (!library) { fprintf(stderr, "%s\n", dlerror()); return 1; }
    PFN_vkCreateInstance create = (PFN_vkCreateInstance)dlsym(library, "vkCreateInstance");
    PFN_vkEnumeratePhysicalDevices enumerate = (PFN_vkEnumeratePhysicalDevices)dlsym(library, "vkEnumeratePhysicalDevices");
    PFN_vkDestroyInstance destroy = (PFN_vkDestroyInstance)dlsym(library, "vkDestroyInstance");
    if (!create || !enumerate || !destroy) return 2;
    VkInstanceCreateInfo info = { .sType = VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO };
    VkInstance instance;
    VkResult result = create(&info, NULL, &instance);
    if (result != VK_SUCCESS) { fprintf(stderr, "vkCreateInstance: %d\n", result); return 3; }
    uint32_t count = 0;
    result = enumerate(instance, &count, NULL);
    destroy(instance, NULL);
    dlclose(library);
    printf("Vulkan enumeration: result=%d, GPUs=%u\n", result, count);
    return result == VK_SUCCESS && count > 0 ? 0 : 4;
}
''')
        probe = temporary / 'vulkan-probe'
        subprocess.run(['cc', str(source), '-ldl', '-o', str(probe)], check=True)
        # Use precisely the reader's loader and library search path. This tests
        # dlopened ICDs/layers, which --version and ldd alone cannot exercise.
        # Keep dynamic-linker tracing to verify dlopened host libraries too.
        env['LD_LIBRARY_PATH'] = str(runtime / 'lib')
        env['VK_LOADER_DEBUG'] = 'error,warn'
        result = subprocess.run(
            [loader, '--library-path', str(runtime / 'lib'), str(probe)],
            env=env, text=True, capture_output=True, timeout=60)
        assert result.returncode == 0, result.stderr
        for line in result.stderr.splitlines():
            if 'calling init:' in line:
                path = line.split('calling init:', 1)[1].strip()
                if pathlib.Path(path).name.startswith(('libwayland-', 'libvulkan.so')):
                    assert str(runtime) not in path, path
        print(result.stdout.strip())
