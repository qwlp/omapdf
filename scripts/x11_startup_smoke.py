#!/usr/bin/env python3
"""Verify a real X11 window opens and survives offline (run under xvfb-run).

Requires xdotool and libseccomp. Settings and caches are isolated; only the
test reader is stopped. No physical desktop focus or cursor is changed.
"""
import argparse
import ctypes
import errno
import os
from pathlib import Path
import signal
import socket
import subprocess
import tempfile
import time


def deny_network():
    """Deny Internet sockets across exec, keeping Unix display/IPC sockets usable."""
    class ArgCmp(ctypes.Structure):
        _fields_ = [('arg', ctypes.c_uint), ('op', ctypes.c_int),
                    ('datum_a', ctypes.c_uint64), ('datum_b', ctypes.c_uint64)]

    seccomp = ctypes.CDLL('libseccomp.so.2')
    seccomp.seccomp_init.argtypes = [ctypes.c_uint32]
    seccomp.seccomp_init.restype = ctypes.c_void_p
    seccomp.seccomp_syscall_resolve_name.argtypes = [ctypes.c_char_p]
    seccomp.seccomp_syscall_resolve_name.restype = ctypes.c_int
    seccomp.seccomp_rule_add_array.argtypes = [ctypes.c_void_p, ctypes.c_uint32,
                                              ctypes.c_int, ctypes.c_uint,
                                              ctypes.POINTER(ArgCmp)]
    seccomp.seccomp_load.argtypes = [ctypes.c_void_p]
    seccomp.seccomp_release.argtypes = [ctypes.c_void_p]
    context = seccomp.seccomp_init(0x7fff0000)  # SCMP_ACT_ALLOW
    assert context, 'Cannot initialize network sandbox'
    try:
        syscall = seccomp.seccomp_syscall_resolve_name(b'socket')
        assert syscall >= 0
        for family in (socket.AF_INET, socket.AF_INET6):
            comparison = ArgCmp(0, 4, family, 0)  # SCMP_CMP_EQ
            result = seccomp.seccomp_rule_add_array(
                context, 0x00050000 | errno.EPERM, syscall, 1,
                ctypes.byref(comparison))
            assert result == 0, result
        assert seccomp.seccomp_load(context) == 0, 'Cannot load network sandbox'
    finally:
        seccomp.seccomp_release(context)
    # Prove the restriction is active before executing the reader.
    for family in (socket.AF_INET, socket.AF_INET6):
        try:
            probe = socket.socket(family, socket.SOCK_STREAM)
        except OSError as error:
            assert error.errno == errno.EPERM, error
        else:
            probe.close()
            raise AssertionError('Internet sockets are still permitted')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--expect-software-renderer', action='store_true')
    args = parser.parse_args()
    assert os.environ.get('DISPLAY'), 'Run this check under xvfb-run'
    with tempfile.TemporaryDirectory(prefix='omapdf-x11-offline-') as temporary:
        directory = Path(temporary)
        env = dict(os.environ)
        for variable in ('WAYLAND_DISPLAY', 'ZED_HEADLESS'):
            env.pop(variable, None)
        for variable in ('XDG_CONFIG_HOME', 'XDG_CACHE_HOME', 'XDG_DATA_HOME',
                         'XDG_RUNTIME_DIR'):
            path = directory / variable.lower()
            path.mkdir(mode=0o700)
            env[variable] = str(path)
        # The startup path must work without documents, login or network.
        with (directory / 'startup.log').open('w+') as log:
            process = subprocess.Popen(
                [str(args.binary.resolve()), '--config', str(directory / 'config'),
                 '--library', str(directory / 'library.json')],
                env=env, stdout=log, stderr=subprocess.STDOUT,
                preexec_fn=deny_network, start_new_session=True)
            try:
                deadline = time.monotonic() + 30
                window = None
                while time.monotonic() < deadline:
                    assert process.poll() is None, 'Reader exited before opening a window'
                    # AppImage runtimes may fork AppRun rather than exec it.
                    pending = [process.pid]
                    while pending:
                        pid = pending.pop()
                        children = Path(f'/proc/{pid}/task/{pid}/children')
                        try:
                            pending.extend(map(int, children.read_text().split()))
                        except FileNotFoundError:
                            pass
                        result = subprocess.run(
                            ['xdotool', 'search', '--onlyvisible', '--pid', str(pid)],
                            env=env, text=True, capture_output=True, timeout=5)
                        if result.returncode == 0 and result.stdout.strip():
                            window = result.stdout.splitlines()[0]
                            break
                    if window:
                        break
                    time.sleep(0.1)
                assert window, 'No visible reader window within 30 seconds'
                # The catalog waits 11 seconds; a mapped window alone must not
                # hide a renderer failure immediately after creation.
                deadline = time.monotonic() + 12
                while time.monotonic() < deadline:
                    assert process.poll() is None, 'Reader exited after opening its window'
                    time.sleep(0.2)
                subprocess.run(['xdotool', 'getwindowgeometry', window], env=env,
                               check=True, capture_output=True, timeout=5)
                if args.expect_software_renderer:
                    log.flush()
                    log.seek(0)
                    assert 'using bundled software Vulkan renderer' in log.read(), 'CPU fallback was not selected'
                print('X11 window opened and survived 12 seconds with IPv4/IPv6 sockets denied')
            except Exception:
                log.flush()
                log.seek(0)
                print(log.read())
                raise
            finally:
                try:
                    os.killpg(process.pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass
                if process.poll() is None:
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        os.killpg(process.pid, signal.SIGKILL)
                        process.wait(timeout=5)


if __name__ == '__main__':
    main()
