#!/usr/bin/env python3
"""Exercise only this application's window on Hyprland; restore prior focus afterward.

Requires a running Hyprland with the Lua API, grim, and a built debug binary.
Uses the API documented at https://wiki.hypr.land/configuring/core/dispatchers/.
"""
import argparse
import json
import pathlib
import subprocess
import tempfile
import time

parser = argparse.ArgumentParser()
parser.add_argument('pdf', type=pathlib.Path)
parser.add_argument('--screenshot', type=pathlib.Path)
parser.add_argument('--binary', type=pathlib.Path, default=pathlib.Path('target/debug/omapdf'))
args = parser.parse_args()

def hypr(*args):
    return subprocess.check_output(['hyprctl', *args], text=True)

def dispatch(expression):
    hypr('eval', f'hl.dispatch({expression})')

prior = json.loads(hypr('activewindow', '-j')).get('address')
with tempfile.TemporaryDirectory(prefix='omapdf-smoke-') as temporary:
    library = pathlib.Path(temporary) / 'library.json'
    configuration = pathlib.Path(temporary) / 'config'
    configuration.mkdir()
    preferences = configuration / 'prefs_user.config'
    preferences.write_text('# Preserved user comment\nunported_preference keep-me\n')
    log = open(pathlib.Path(temporary) / 'gui.log', 'w+')
    process = subprocess.Popen([
        str(args.binary), '--library', str(library), '--config', str(configuration), str(args.pdf.resolve()),
        '--execute-command', 'goto 2', '--execute-command', 'add_bookmark smoke-test',
    ], stdout=log, stderr=subprocess.STDOUT)
    try:
        deadline = time.monotonic() + 15
        window = None
        while time.monotonic() < deadline:
            assert process.poll() is None, 'reader exited before opening a document'
            windows = json.loads(hypr('clients', '-j'))
            window = next((w for w in windows if w.get('pid') == process.pid), None)
            if window and library.exists():
                state = json.loads(library.read_text())
                if state['bookmarks']:
                    break
            time.sleep(0.1)
        else:
            raise AssertionError('reader did not finish loading within 15 seconds')
        assert not window['xwayland'], 'reader used XWayland'
        address = 'address:' + window['address']
        dispatch('hl.dsp.focus({window=' + json.dumps(address) + '})')

        def key(key, mods=''):
            dispatch('hl.dsp.send_shortcut({window=' + json.dumps(address)
                     + ',mods=' + json.dumps(mods) + ',key=' + json.dumps(key) + '})')
            time.sleep(0.06)

        def remote(command):
            subprocess.run([str(args.binary), '--library', str(library), '--execute-command', command], check=True, timeout=5)

        def held_boundary(key_name, command):
            remote(command)
            time.sleep(0.8)
            current = next(w for w in json.loads(hypr('clients', '-j')) if w.get('pid') == process.pid)
            x,y = current['at']; w,h = current['size']
            geometry = f'{x},{y+40} {w}x{max(1,h-80)}'
            def state(value):
                dispatch('hl.dsp.send_key_state({window=' + json.dumps(address) + ',mods="",key=' + json.dumps(key_name) + ',state=' + json.dumps(value) + '})')
            state('down')
            try:
                time.sleep(0.5)
                frames = []
                for _ in range(4):
                    frames.append(subprocess.check_output(['grim','-g',geometry,'-']))
                    time.sleep(0.12)
                if not all(frame == frames[0] for frame in frames):
                    for index,frame in enumerate(frames):
                        pathlib.Path(f'/tmp/omapdf-boundary-{key_name}-{index}.png').write_bytes(frame)
                assert all(frame == frames[0] for frame in frames), f'held {key_name} flickered at document boundary'
            finally:
                state('up')

        # Force a page taller than the viewport, then exercise actual scrolling.
        # Boundary-only tests never enter the animation-scheduling path.
        key('Escape')
        remote('goto 1;zoom_in;zoom_in;zoom_in;zoom_in')
        time.sleep(0.5)
        current = next(w for w in json.loads(hypr('clients', '-j')) if w.get('pid') == process.pid)
        x,y = current['at']; w,h = current['size']
        def active_frame():
            return subprocess.check_output(['grim', '-g', f'{x},{y+40} {w}x{max(1,h-80)}', '-'])
        before = active_frame()
        key('j')
        time.sleep(0.3)
        assert process.poll() is None, 'reader crashed during active j scrolling'
        assert active_frame() != before, 'smooth scrolling survived but did not move the page'
        for direction in ['j', 'k', 'k']:
            key(direction)
            time.sleep(0.15)
            assert process.poll() is None, f'reader crashed during active {direction} scrolling'
        # Also exercises commands delivered while the view is being rendered.
        remote('screen_down')
        time.sleep(0.25)
        assert process.poll() is None, 'reader crashed during command-driven scrolling'
        remote('fit_to_page_width;goto_top_of_page')
        time.sleep(0.3)

        held_boundary('k', 'goto 1;goto_top_of_page')
        held_boundary('j', 'goto_end;goto_bottom_of_page')
        remote('goto 2')
        time.sleep(0.2)

        # Settings use an isolated config directory and retain unknown Omapdf options.
        key('comma','CTRL')
        remote('settings;set_preference smooth_scroll 0;set_preference vertical_move_amount 1.5;set_preference dark_mode_contrast 0.7;set_preference highlight_color_z 0.2 0.4 0.6')
        time.sleep(0.3)
        deadline = time.monotonic() + 3
        while 'smooth_scroll 0' not in preferences.read_text() and time.monotonic() < deadline:
            time.sleep(0.05)
        saved_preferences = preferences.read_text()
        if 'smooth_scroll 0' not in saved_preferences:
            print('Saved settings:', repr(saved_preferences))
            current = next(w for w in json.loads(hypr('clients','-j')) if w.get('pid') == process.pid)
            x,y = current['at']; w,h = current['size']
            subprocess.run(['grim','-g',f'{x},{y} {w}x{h}','/tmp/omapdf-settings-failure.png'],check=True)
        assert 'smooth_scroll 0' in saved_preferences
        assert 'vertical_move_amount 1.5' in saved_preferences
        assert 'unported_preference keep-me' in saved_preferences
        assert '# Preserved user comment' in saved_preferences
        if args.screenshot:
            current = next(w for w in json.loads(hypr('clients','-j')) if w.get('pid') == process.pid)
            x,y = current['at']; w,h = current['size']
            settings_image = args.screenshot.with_stem(args.screenshot.stem + '-settings')
            subprocess.run(['grim','-g',f'{x},{y} {w}x{h}',str(settings_image)],check=True)
        remote('reset_preferences;reload_config;close')
        time.sleep(0.2)
        assert 'smooth_scroll 1' in preferences.read_text()
        assert 'unported_preference keep-me' in preferences.read_text()

        # Input goes through the Wayland keyboard and GPUI's platform text handler.
        key('semicolon', 'SHIFT')
        for char in 'goto 3':
            key('space' if char == ' ' else char)
        key('Return')
        time.sleep(0.3)
        state = json.loads(library.read_text())
        doc = next(iter(state['documents'].values()))
        assert doc['page'] == 2, f'command prompt did not navigate to page 3: {doc["page"]}'
        assert state['bookmarks'][0]['name'] == 'smoke-test'
        subprocess.run([str(args.binary), '--library', str(library), '--execute-command', 'goto 4'], check=True, timeout=5)
        time.sleep(0.3)
        assert next(iter(json.loads(library.read_text())['documents'].values()))['page'] == 3, 'IPC navigation failed'
        key('p', 'CTRL')
        for char in 'gotobeg':
            key(char)
        key('Return')
        time.sleep(0.3)
        assert next(iter(json.loads(library.read_text())['documents'].values()))['page'] == 0, 'fuzzy command palette failed'
        key('slash')
        for char in 'omapdf':
            key(char)
        key('Return')
        time.sleep(0.4)
        remote('add_highlight a;close')
        time.sleep(0.2)
        state = json.loads(library.read_text())
        doc = next(iter(state['documents'].values()))
        assert doc['highlights'], 'search selection could not be highlighted'
        assert doc['highlights'][0]['kind'] == 'a'
        key('F8')
        time.sleep(0.6)
        if args.screenshot:
            window = next(w for w in json.loads(hypr('clients', '-j')) if w.get('pid') == process.pid)
            x, y = window['at']
            w, h = window['size']
            subprocess.run(['grim', '-g', f'{x},{y} {w}x{h}', str(args.screenshot)], check=True)
        key('Escape')
        key('p')
        subprocess.run([str(args.binary), '--library', str(library), '--execute-command', 'goto 5'], check=True, timeout=5)
        time.sleep(0.3)
        key('p')
        key('F12')
        time.sleep(0.8)
        helpers = [w for w in json.loads(hypr('clients', '-j')) if w.get('class') == 'io.omapdf.helper']
        assert len(helpers) == 1 and not helpers[0]['xwayland'], 'native portal helper did not open'
        key('F12')
        time.sleep(0.2)
        subprocess.run([str(args.binary), '--library', str(library), '--execute-command', 'goto_portal'], check=True, timeout=5)
        time.sleep(0.3)
        state = json.loads(library.read_text())
        doc = next(iter(state['documents'].values()))
        assert doc['portals'] and doc['portals'][0]['destination']['page'] == 4
        key('F12')
        time.sleep(0.3)
        key('q', 'CTRL')
        process.wait(timeout=5)
        assert process.returncode == 0
        modified = library.stat().st_mtime_ns
        saved_page = next(iter(json.loads(library.read_text())['documents'].values()))['page']
        process = subprocess.Popen([str(args.binary), '--library', str(library), '--config', str(configuration), str(args.pdf.resolve())], stdout=log, stderr=subprocess.STDOUT)
        deadline = time.monotonic() + 10
        while library.stat().st_mtime_ns == modified and time.monotonic() < deadline:
            time.sleep(0.1)
        assert library.stat().st_mtime_ns != modified, 'reopened reader did not load'
        restored_page = next(iter(json.loads(library.read_text())['documents'].values()))['page']
        assert restored_page == saved_page, 'saved reading position was not restored'
        subprocess.run([str(args.binary), '--library', str(library), '--execute-command', 'quit'], check=True, timeout=5)
        process.wait(timeout=5)
        assert process.returncode == 0
        print('PASS: native Wayland, PDF load, startup commands, typed prompt, IPC, fuzzy palette, search, highlight, dark mode, portals, native helper, settings persistence, active smooth scrolling, held j/k boundary stability, saved position restoration, clean quit')
    finally:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=5)
        log.seek(0)
        output = log.read()
        if output:
            print(output)
        if prior:
            dispatch('hl.dsp.focus({window=' + json.dumps('address:' + prior) + '})')
