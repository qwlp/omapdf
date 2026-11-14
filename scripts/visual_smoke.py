#!/usr/bin/env python3
"""Exercise the text ruler in an isolated native Wayland reader; restore focus/cursor."""
import argparse
import io
import json
import pathlib
import subprocess
import tempfile
import time
from PIL import Image, ImageChops

parser = argparse.ArgumentParser()
parser.add_argument('--binary', default='target/release/omapdf')
parser.add_argument('--screenshots', type=pathlib.Path)
args = parser.parse_args()


def hypr(*arguments):
    return subprocess.check_output(['hyprctl', *arguments], text=True)


def dispatch(expression):
    return hypr('eval', 'hl.dispatch(' + expression + ')')


prior = json.loads(hypr('activewindow', '-j')).get('address')
cursor = json.loads(hypr('cursorpos', '-j'))
with tempfile.TemporaryDirectory(prefix='omapdf-visual-') as temporary:
    root = pathlib.Path(temporary)
    config = root / 'config'
    config.mkdir()
    (config / 'prefs_user.config').write_text('ui_animations 0\nmax_page_width 700\n')
    (config / 'keys_user.config').write_text(
        'enter_visual_mark_mode r\nmove_visual_mark_down x\nmove_visual_mark_up y\n')
    objects = [
        '<< /Type /Catalog /Pages 2 0 R >>',
        '<< /Type /Pages /Kids [3 0 R 4 0 R 5 0 R] /Count 3 >>',
        '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 400] /Resources << /Font << /F1 6 0 R >> >> /Contents 7 0 R >>',
        '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 400] /Resources << >> >>',
        '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 400] /Resources << /Font << /F1 6 0 R >> >> /Contents 7 0 R >>',
        '<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>',
    ]
    stream = 'BT /F1 16 Tf 30 330 Td (First line) Tj 0 -70 Td (Second line) Tj 0 -70 Td (Third line) Tj ET\n'
    objects.append(f'<< /Length {len(stream)} >>\nstream\n{stream}endstream')
    data = '%PDF-1.4\n'
    offsets = []
    for index, obj in enumerate(objects):
        offsets.append(len(data))
        data += f'{index+1} 0 obj\n{obj}\nendobj\n'
    xref = len(data)
    data += f'xref\n0 {len(objects)+1}\n0000000000 65535 f \n'
    for offset in offsets:
        data += f'{offset:010} 00000 n \n'
    data += f'trailer\n<< /Size {len(objects)+1} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n'
    pdf = root / 'ruler.pdf'
    pdf.write_text(data)
    library = root / 'library.json'
    base = [args.binary, '--library', str(library), '--config', str(config)]
    log = open(root / 'gui.log', 'w+')
    process = subprocess.Popen(base + [str(pdf)], stdout=log, stderr=subprocess.STDOUT)

    def clients():
        return [w for w in json.loads(hypr('clients', '-j')) if w.get('pid') == process.pid]

    def state():
        return json.loads(library.read_text()) if library.exists() else {}

    def wait(test):
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            if process.poll() is not None:
                log.seek(0)
                raise AssertionError(log.read())
            if test():
                return
            time.sleep(.05)
        log.seek(0)
        raise AssertionError('timeout: ' + log.read())

    def remote(command):
        subprocess.run(base + ['--execute-command', command], check=True, timeout=5)
        time.sleep(.25)

    def key(value):
        dispatch('hl.dsp.send_shortcut({window=' + json.dumps(address) + ',mods="",key=' + json.dumps(value) + '})')
        time.sleep(.06)

    def repeat(value, count):
        for digit in str(count):
            key(digit)
        key(value)
        time.sleep(.35)

    def capture(name):
        window = clients()[0]
        x, y = window['at']
        width, height = window['size']
        picture = Image.open(io.BytesIO(subprocess.check_output([
            'grim', '-g', f'{x},{y+45} {width}x{height-80}', '-']))).convert('RGB')
        if args.screenshots:
            args.screenshots.mkdir(parents=True, exist_ok=True)
            picture.save(args.screenshots / (name + '.png'))
        return picture

    try:
        wait(lambda: clients() and state().get('documents'))
        address = 'address:' + clients()[0]['address']
        assert not clients()[0]['xwayland']
        dispatch('hl.dsp.focus({window=' + json.dumps(address) + '})')
        remote('fit_to_page;close_visual_mark')
        time.sleep(.6)
        plain = capture('plain')
        key('r')
        repeat('y', 99999)
        first = capture('first-line')
        assert ImageChops.difference(plain, first).getbbox(), 'ruler was not painted'
        key('j')
        time.sleep(.35)
        second = capture('second-line')
        assert ImageChops.difference(first, second).getbbox(), 'j did not move ruler'
        key('k')
        time.sleep(.35)
        assert ImageChops.difference(first, capture('first-again')).getbbox() is None, 'k did not return to the previous line'
        remote('close_visual_mark')
        assert ImageChops.difference(plain, capture('closed')).getbbox() is None, 'closing ruler left an overlay'
        for layout in ['single', 'spread', 'horizontal', 'vertical']:
            remote('layout_' + layout + ';goto 1;enter_visual_mark_mode')
            repeat('x', 99999)
            remote('add_bookmark last-' + layout)
            wait(lambda: state()['bookmarks'][-1]['name'] == 'last-' + layout)
            assert state()['bookmarks'][-1]['location']['page'] == 2, 'ruler did not cross blank page'
            repeat('y', 99999)
            remote('add_bookmark first-' + layout)
            wait(lambda: state()['bookmarks'][-1]['name'] == 'first-' + layout)
            assert state()['bookmarks'][-1]['location']['page'] == 0, 'ruler did not clamp to beginning'
            key('Escape')
        # Closing and navigating cancel in-flight ruler work.
        remote('enter_visual_mark_mode;move_visual_mark_down;close_visual_mark;goto 2')
        time.sleep(.5)
        remote('add_bookmark cancelled')
        assert state()['bookmarks'][-1]['location']['page'] == 1
        remote('overview 3')
        time.sleep(.8)
        remote('portal_to_overview')
        wait(lambda: len(next(iter(state()['documents'].values()))['portals']) == 1)
        remote('close_overview;portal_to_overview')
        assert len(next(iter(state()['documents'].values()))['portals']) == 1, 'closed overview retained its destination'
        remote('goto_portal;add_bookmark preview-portal')
        assert state()['bookmarks'][-1]['location']['page'] == 2
        remote('overview 1;close')
        time.sleep(.8)
        remote('portal_to_overview')
        assert len(next(iter(state()['documents'].values()))['portals']) == 1, 'closed overview reopened after rendering'
        remote('quit')
        process.wait(timeout=5)
        assert process.returncode == 0
        print('PASS: ruler pixels, real j/k and numeric prefixes, four layouts, blank-page traversal, boundaries, cancellation, overview portal creation/dismissal, clean quit')
    finally:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=5)
        if process.returncode:
            log.seek(0)
            print(log.read())
        dispatch('hl.dsp.cursor.move({x=' + str(cursor['x']) + ',y=' + str(cursor['y']) + '})')
        if prior:
            dispatch('hl.dsp.focus({window=' + json.dumps('address:' + prior) + '})')
