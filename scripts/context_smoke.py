#!/usr/bin/env python3
"""Exercise pointer selection and the real right-click menu in an isolated Wayland reader."""
import argparse,json,pathlib,subprocess,tempfile,time
parser=argparse.ArgumentParser();parser.add_argument('pdf',type=pathlib.Path)
parser.add_argument('--binary',default='target/release/omapdf');parser.add_argument('--screenshot',type=pathlib.Path)
args=parser.parse_args()
def hypr(*args):return subprocess.check_output(['hyprctl',*args],text=True)
def dispatch(expression):hypr('eval','hl.dispatch('+expression+')')
prior=json.loads(hypr('activewindow','-j')).get('address')
prior_cursor=json.loads(hypr('cursorpos','-j'))
with tempfile.TemporaryDirectory(prefix='omapdf-context-') as temporary:
    root=pathlib.Path(temporary);config=root/'config';config.mkdir();library=root/'library.json';log=open(root/'gui.log','w+')
    process=subprocess.Popen([args.binary,'--library',str(library),'--config',str(config),str(args.pdf.resolve())],stdout=log,stderr=subprocess.STDOUT)
    def state():return json.loads(library.read_text())
    def document():return next(iter(state()['documents'].values()))
    def remote(command):subprocess.run([args.binary,'--library',str(library),'--execute-command',command],check=True,timeout=5)
    def key(value,mods=''):dispatch('hl.dsp.send_shortcut({window='+json.dumps(address)+',mods='+json.dumps(mods)+',key='+json.dumps(value)+'})');time.sleep(.06)
    def wait_for(test):
        end=time.monotonic()+15
        while time.monotonic()<end:
            if process.poll() is not None:
                log.seek(0);raise AssertionError('reader exited: '+log.read())
            if test():return
            time.sleep(.05)
        log.seek(0);raise AssertionError('timeout: '+log.read())
    try:
        wait_for(lambda:library.exists() and bool(state().get('documents')) and any(w.get('pid')==process.pid for w in json.loads(hypr('clients','-j'))))
        window=next(w for w in json.loads(hypr('clients','-j')) if w.get('pid')==process.pid)
        address='address:'+window['address'];dispatch('hl.dsp.focus({window='+json.dumps(address)+'})');time.sleep(.6)
        remote('goto 1;search commands;close');time.sleep(.8)
        # Open beside the selected word, then activate Highlight with keyboard navigation.
        remote('context_menu');time.sleep(.3)
        if True:
            window=next(w for w in json.loads(hypr('clients','-j')) if w.get('pid')==process.pid)
            x,y=window['at'];w,h=window['size'];subprocess.run(['grim','-g',f'{x},{y} {w}x{h}',str(args.screenshot or root/'menu.png')],check=True)
        # Locate the actual floating panel, then click Highlight with the mouse.
        # This catches click-through bugs that keyboard-only activation misses.
        from PIL import Image
        from collections import defaultdict
        picture=Image.open(args.screenshot or root/'menu.png').convert('RGB')
        runs=defaultdict(list)
        for row in range(40,picture.height-10):
            start=None
            for col in range(picture.width):
                dark=max(picture.getpixel((col,row)))<65
                if dark and start is None:start=col
                elif not dark and start is not None:
                    width=col-start
                    if 258<=width<=264:runs[(start,width)].append(row)
                    start=None
        (left,width),rows=max(runs.items(),key=lambda item:len(item[1]))
        top=min(rows)
        assert len(rows)>100, 'context menu was not painted'
        dispatch('hl.dsp.cursor.move({x='+str(x+left+80)+',y='+str(y+top+79)+'})');time.sleep(.08)
        key('mouse:272')
        wait_for(lambda:len(document()['highlights'])==1)
        assert document()['highlights'][0]['text'].lower().find('commands')>=0
        # Real mouse right-click in a PDF margin. Keyboard navigation reaches
        # Bookmark this page (Copy is disabled), and persists it in the library.
        remote('goto 1;fit_to_page_width');time.sleep(.4)
        key('Escape')
        window=next(w for w in json.loads(hypr('clients','-j')) if w.get('pid')==process.pid)
        x,y=window['at'];dispatch('hl.dsp.cursor.move({x='+str(x+80)+',y='+str(y+92)+'})');time.sleep(.1)
        key('mouse:273')
        time.sleep(.2)
        key('Return')
        for letter in 'context-check':key('minus' if letter=='-' else letter)
        key('Return')
        wait_for(lambda:any(b['name']=='context-check' for b in state()['bookmarks']))
        remote('quit');process.wait(timeout=5);assert process.returncode==0
        print('PASS: context selection, mouse highlight action without click-through, real right-click and bookmark action, clean quit')
    finally:
        if process.poll() is None:process.terminate();process.wait(timeout=5)
        if process.returncode!=0:log.seek(0);print(log.read())
        if prior:dispatch('hl.dsp.focus({window='+json.dumps('address:'+prior)+'})')
        dispatch('hl.dsp.cursor.move({x='+str(prior_cursor['x'])+',y='+str(prior_cursor['y'])+'})')
