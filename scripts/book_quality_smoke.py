#!/usr/bin/env python3
"""Inspect native page rendering and cleaned outlines of the original CC0 PDF fixture on Wayland."""
import argparse,json,pathlib,subprocess,tempfile,time
parser=argparse.ArgumentParser();parser.add_argument('--binary',default='dist/omapdf');args=parser.parse_args()
book=pathlib.Path('examples/reading-lab.pdf').resolve()
def hypr(*args):return subprocess.check_output(['hyprctl',*args],text=True)
def dispatch(expr):return hypr('eval','hl.dispatch('+expr+')')
prior=json.loads(hypr('activewindow','-j')).get('address')
cursor=json.loads(hypr('cursorpos','-j'))
with tempfile.TemporaryDirectory(prefix='omapdf-quality-') as temporary:
    root=pathlib.Path(temporary);library=root/'library.json';log=open(root/'log','w+')
    base=[args.binary,'--library',str(library),'--config',str(root/'config')]
    process=subprocess.Popen(base+[str(book),'--page','30'],stdout=log,stderr=subprocess.STDOUT)
    def clients():return [w for w in json.loads(hypr('clients','-j')) if w.get('pid')==process.pid]
    def wait(test):
        end=time.monotonic()+20
        while time.monotonic()<end:
            if process.poll() is not None:log.seek(0);raise AssertionError(log.read())
            if test():return
            time.sleep(.1)
        log.seek(0);raise AssertionError('timeout: '+log.read())
    def remote(command):subprocess.run(base+['--execute-command',command],check=True,timeout=5)
    def capture(path):
        w=clients()[0];x,y=w['at'];width,height=w['size']
        subprocess.run(['grim','-g',f'{x},{y} {width}x{height}',path],check=True)
    try:
        wait(lambda:clients() and library.exists() and bool(json.loads(library.read_text()).get('documents')))
        address='address:'+clients()[0]['address'];dispatch('hl.dsp.focus({window='+json.dumps(address)+'})')
        remote('set_preference max_page_width 930;fit_to_page_width;goto 30;goto_top_of_page');time.sleep(1.)
        capture('/tmp/omapdf-book-native.png')
        assert next(iter(json.loads(library.read_text())['documents'].values()))['page']==29
        remote('goto_toc');time.sleep(.5)
        # Narrow to a nested section title; ancestor entries must still navigate.
        for ch in 'history':
            dispatch('hl.dsp.send_shortcut({window='+json.dumps(address)+',mods="",key='+json.dumps(ch)+'})');time.sleep(.06)
        capture('/tmp/omapdf-book-contents.png')
        remote('settings');time.sleep(.4);capture('/tmp/omapdf-book-omarchy-font.png')
        remote('quit');process.wait(timeout=5);assert process.returncode==0
        print('PASS: original CC0 PDF fixture, page 30 native rendering, filtered nested contents, Omarchy-font settings, clean quit')
    finally:
        if process.poll() is None:process.terminate();process.wait(timeout=5)
        if prior:dispatch('hl.dsp.focus({window='+json.dumps('address:'+prior)+'})')
        dispatch('hl.dsp.cursor.move({x='+str(cursor['x'])+',y='+str(cursor['y'])+'})')
