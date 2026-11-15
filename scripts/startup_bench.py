#!/usr/bin/env python3
"""Compare startup of release/portable readers on Hyprland, with isolated state.

Example: python3 scripts/startup_bench.py --binary before=/tmp/old-omapdf \
    --binary after=dist/omapdf --runs 7 --output /tmp/startup.json

Window timing measures compositor registration, not physical display latency.
Ready timing measures a startup bookmark command; it includes command scheduling.
Fresh extraction uses a new cache, but does not flush OS or GPU driver caches.
Only benchmark processes are stopped. Desktop focus and cursor are restored.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parent.parent


def hypr(*args):
    return subprocess.check_output(['hyprctl', *args], text=True)


def version(binary, env):
    start = time.perf_counter()
    subprocess.run([str(binary), '--version'], env=env, capture_output=True,
                   check=True, timeout=30)
    return (time.perf_counter() - start) * 1000


def launch(binary, document, root, env):
    root.mkdir()
    config = root / 'config'
    config.mkdir()
    library = root / 'library.json'
    command = [str(binary), '--library', str(library), '--config', str(config)]
    if document:
        command += [str(document), '--execute-command', 'add_bookmark startup-probe']
    with (root / 'gui.log').open('w+') as log:
        start = time.perf_counter()
        process = subprocess.Popen(command, env=env, stdout=log, stderr=log)
        mapped = ready = None
        try:
            while time.perf_counter() - start < 20:
                if process.poll() is not None:
                    log.seek(0)
                    raise RuntimeError('Reader exited during startup: ' + log.read())
                if mapped is None:
                    clients = json.loads(hypr('clients', '-j'))
                    if any(client.get('pid') == process.pid for client in clients):
                        mapped = (time.perf_counter() - start) * 1000
                if document and library.exists():
                    try:
                        if json.loads(library.read_text()).get('bookmarks'):
                            ready = ready or (time.perf_counter() - start) * 1000
                    except (OSError, ValueError):
                        pass
                if mapped is not None and (not document or ready is not None):
                    # Keep the window alive for initial rasterization too.
                    time.sleep(0.15)
                    return {'window_ms': mapped, 'ready_ms': ready}
                time.sleep(0.01)
            log.seek(0)
            raise RuntimeError('Startup timed out: ' + log.read())
        finally:
            if process.poll() is None:
                process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', action='append', required=True, metavar='LABEL=PATH')
    parser.add_argument('--runs', type=int, default=7)
    parser.add_argument('--document', action='append', type=Path,
                        help='Override bundled test documents (repeatable)')
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    if args.runs < 1:
        parser.error('--runs must be positive')
    binaries = {label: Path(path).resolve() for label, path in
                (item.split('=', 1) for item in args.binary)}
    documents = [None] + [path.resolve() for path in (args.document or [
        ROOT / 'examples/reading-lab.pdf', ROOT / 'examples/large-reading-lab.pdf',
        ROOT / 'examples/reading-lab.epub'])]
    prior = json.loads(hypr('activewindow', '-j')).get('address')
    cursor = json.loads(hypr('cursorpos', '-j'))
    report = {'runs': args.runs, 'binaries': {k: str(v) for k, v in binaries.items()},
              'binary_sha256': {k: hashlib.sha256(v.read_bytes()).hexdigest()
                                for k, v in binaries.items()},
              'samples': {}, 'medians_ms': {}}
    try:
        with tempfile.TemporaryDirectory(prefix='omapdf-startup-bench-') as temporary:
            root = Path(temporary)
            envs = {label: dict(os.environ, XDG_CACHE_HOME=str(root / label / 'cache'))
                    for label in binaries}
            for label, binary in binaries.items():
                (root / label).mkdir()
                fresh = []
                for run in range(args.runs):
                    fresh.append(version(binary, dict(os.environ,
                        XDG_CACHE_HOME=str(root / label / f'extract-{run}'))))
                version(binary, envs[label])
                cached = [version(binary, envs[label]) for _ in range(args.runs)]
                report['samples'][label] = {'fresh_version_ms': fresh,
                                            'cached_version_ms': cached, 'gui': {}}
                report['medians_ms'][label] = {
                    'fresh_version': statistics.median(fresh),
                    'cached_version': statistics.median(cached), 'gui': {}}
            for index, document in enumerate(documents):
                name = document.name if document else 'empty'
                for label, binary in binaries.items():
                    launch(binary, document, root / label / f'warm-{index}', envs[label])
                    report['samples'][label]['gui'][name] = []
                for run in range(args.runs):
                    # Alternate order to reduce bias from temperature/load changes.
                    order = list(binaries.items())
                    if run % 2:
                        order.reverse()
                    for label, binary in order:
                        sample = launch(binary, document,
                            root / label / f'run-{index}-{run}', envs[label])
                        report['samples'][label]['gui'][name].append(sample)
                for label in binaries:
                    samples = report['samples'][label]['gui'][name]
                    medians = {key: statistics.median(s[key] for s in samples
                               if s[key] is not None) for key in samples[0]
                               if samples[0][key] is not None}
                    report['medians_ms'][label]['gui'][name] = medians
                    print(label, name, {k: round(v, 1) for k, v in medians.items()}, flush=True)
    finally:
        if prior:
            hypr('eval', 'hl.dispatch(hl.dsp.focus({window=' +
                 json.dumps('address:' + prior) + '}))')
        hypr('eval', f'hl.dispatch(hl.dsp.cursor.move({{x={cursor["x"]},y={cursor["y"]}}}))')
    for label, metrics in report['medians_ms'].items():
        print(label, 'fresh extraction/version', round(metrics['fresh_version'], 1),
              'ms; cached version', round(metrics['cached_version'], 1), 'ms')
    if args.output:
        args.output.write_text(json.dumps(report, indent=2) + '\n')


if __name__ == '__main__':
    main()
