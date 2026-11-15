#!/usr/bin/env python3
"""Check optional page separators and scrollbar navigation in an isolated Wayland reader."""
import argparse
import io
import json
import pathlib
import subprocess
import tempfile
import time
from PIL import Image

parser = argparse.ArgumentParser()
parser.add_argument('--binary', default='target/release/omapdf')
args = parser.parse_args()
def hypr(*arguments):
    return subprocess.check_output(['hyprctl', *arguments], text=True)
def dispatch(expression):
    return hypr('eval', 'hl.dispatch(' + expression + ')')
prior = json.loads(hypr('activewindow', '-j')).get('address')
cursor = json.loads(hypr('cursorpos', '-j'))
with tempfile.TemporaryDirectory(prefix='omapdf-navigation-') as temporary:
    root = pathlib.Path(temporary)
    config = root / 'config'
    config.mkdir()
    prefs = config / 'prefs_user.config'
    prefs.write_text('ui_animations 0\npage_layout vertical\nmax_page_width 400\n')
    library = root / 'library.json'
    base = [args.binary, '--config', str(config), '--library', str(library)]
    log = open(root / 'reader.log', 'w+')
    process = subprocess.Popen(base + [str(pathlib.Path('examples/reading-lab.pdf').resolve())], stdout=log, stderr=subprocess.STDOUT)
    address = None
    held = False
    def wait(test):
        deadline = time.monotonic() + 20
        while time.monotonic() < deadline:
            if process.poll() is not None:
                log.seek(0)
                raise AssertionError(log.read())
            if test():
                return
            time.sleep(.05)
        raise AssertionError('reader timeout')
    def clients():
        return [w for w in json.loads(hypr('clients', '-j')) if w.get('pid') == process.pid]
    def remote(command):
        subprocess.run(base + ['--execute-command', command], check=True, timeout=5)
        time.sleep(.4)
    def move(x, y):
        w = clients()[0]
        dispatch('hl.dsp.cursor.move({x=' + str(w['at'][0] + round(x)) + ',y=' + str(w['at'][1] + round(y)) + '})')
        time.sleep(.1)
    def button(state):
        dispatch('hl.dsp.send_key_state({window=' + json.dumps(address) + ',mods="",key="mouse:272",state=' + json.dumps(state) + '})')
        time.sleep(.1)
    def picture():
        w = clients()[0]
        x, y = w['at']
        width, height = w['size']
        return Image.open(io.BytesIO(subprocess.check_output(['grim', '-g', f'{x},{y} {width}x{height}', '-']))).convert('RGB')
    def position(name):
        remote('add_bookmark ' + name)
        state = json.loads(library.read_text())
        return next(b['location'] for b in state['bookmarks'] if b['name'] == name)
    try:
        wait(lambda: clients() and library.exists())
        address = 'address:' + clients()[0]['address']
        dispatch('hl.dsp.focus({window=' + json.dumps(address) + '})')
        remote('goto 1;goto_top_of_page')
        before = picture()
        remote('set_preference page_separators 1')
        separators = picture()
        assert before.tobytes() != separators.tobytes(), 'page separator not visible'
        remote('set_preference scrollbar 1')
        shown = picture()
        assert separators.tobytes() != shown.tobytes(), 'scrollbar not visible'
        assert 'page_separators 1' in prefs.read_text() and 'scrollbar 1' in prefs.read_text()
        start = position('start')
        width, height = clients()[0]['size']
        move(width - 6, height * .75)
        button('down')
        held = True
        button('up')
        held = False
        clicked = position('clicked')
        assert clicked != start, 'scrollbar track click did not navigate'
        remote('goto 1;goto_top_of_page')
        move(width - 6, 80)
        button('down')
        held = True
        move(width - 6, height * .5)
        button('up')
        held = False
        dragged = position('dragged')
        assert dragged != start, 'scrollbar thumb drag did not navigate'
        remote('set_preference page_layout horizontal;goto 1;goto_top_of_page')
        horizontal_start = position('horizontal-start')
        move(width * .7, height - 6)
        button('down')
        held = True
        button('up')
        held = False
        assert position('horizontal-clicked') != horizontal_start, 'horizontal scrollbar did not navigate'
        remote('set_preference scrollbar 0;set_preference page_separators 0;reload_config')
        assert 'scrollbar 0' in prefs.read_text() and 'page_separators 0' in prefs.read_text()
        remote('quit')
        process.wait(timeout=5)
        assert process.returncode == 0
        print('PASS: separators, scrollbar visibility, saved toggles, vertical click/drag, horizontal navigation')
    finally:
        if held and address:
            button('up')
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=5)
        dispatch('hl.dsp.cursor.move({x=' + str(cursor['x']) + ',y=' + str(cursor['y']) + '})')
        if prior:
            dispatch('hl.dsp.focus({window=' + json.dumps('address:' + prior) + '})')
