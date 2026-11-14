#!/usr/bin/env python3
"""Exercise all reader layouts with isolated state and IPC; optional own-window captures."""
import argparse,json,pathlib,subprocess,tempfile,time
parser=argparse.ArgumentParser()
parser.add_argument('pdf',type=pathlib.Path)
parser.add_argument('--binary',default='target/debug/omapdf')
parser.add_argument('--screenshots',type=pathlib.Path)
args=parser.parse_args()
def hypr(*arguments):return subprocess.check_output(['hyprctl',*arguments],text=True)
def focus(address):hypr('eval','hl.dispatch(hl.dsp.focus({window='+json.dumps('address:'+address)+'}))')
prior=json.loads(hypr('activewindow','-j')).get('address')
with tempfile.TemporaryDirectory(prefix='omapdf-layout-') as temporary:
    root=pathlib.Path(temporary);config=root/'config';config.mkdir();library=root/'library.json'
    preferences=config/'prefs_user.config';log=open(root/'gui.log','w+')
    process=subprocess.Popen([args.binary,'--library',str(library),'--config',str(config),str(args.pdf.resolve())],stdout=log,stderr=subprocess.STDOUT)
    def remote(command):subprocess.run([args.binary,'--library',str(library),'--execute-command',command],check=True,timeout=5)
    def state():return json.loads(library.read_text())
    def document():return next(iter(state()['documents'].values()))
    def wait_for(test):
        end=time.monotonic()+10
        while time.monotonic()<end:
            if process.poll() is not None:
                log.seek(0);raise AssertionError('reader exited: '+log.read())
            if test():return
            time.sleep(.05)
        log.seek(0);raise AssertionError('timed out; '+log.read())
    def capture(mode):
        if not args.screenshots:return
        args.screenshots.mkdir(parents=True,exist_ok=True)
        previous=json.loads(hypr('activewindow','-j')).get('address')
        try:
            window=next(w for w in json.loads(hypr('clients','-j')) if w.get('pid')==process.pid)
            focus(window['address']);time.sleep(.6)
            remote(f'layout_{mode};goto 1;fit_to_page_width;close')
            time.sleep(.6)
            if mode in ['horizontal','vertical']:
                remote('screen_down;screen_down')
                time.sleep(.6)
                remote(f'add_bookmark strip-{mode}')
                wait_for(lambda:bool(state()['bookmarks']) and state()['bookmarks'][-1]['name']==f'strip-{mode}')
                offset=state()['bookmarks'][-1]['location']['offset']
                axis=0 if mode=='horizontal' else 1
                assert abs(offset[axis])>.05,f'{mode} strip did not scroll along its primary axis: {offset}'
            window=next(w for w in json.loads(hypr('clients','-j')) if w.get('pid')==process.pid)
            x,y=window['at'];w,h=window['size'];path=args.screenshots/(mode+'.png')
            subprocess.run(['grim','-g',f'{x},{y} {w}x{h}',str(path)],check=True)
            if mode=='spread':
                from PIL import Image
                image=Image.open(path).convert('RGB')
                row=[min(image.getpixel((x,100)))>245 for x in range(image.width)]
                lengths=[];length=0
                for white in row+[False]:
                    if white:length+=1
                    else:
                        if length>40:lengths.append(length)
                        length=0
                assert len(lengths)==2,f'two-page mode did not display two distinct pages: {lengths}'
        finally:
            if previous:focus(previous)
    try:
        wait_for(lambda:library.exists() and bool(state().get('documents')))
        if prior:focus(prior)
        for mode in ['single','spread','horizontal','vertical']:
            remote(f'layout_{mode};goto 1')
            wait_for(lambda:preferences.exists() and f'page_layout {mode}' in preferences.read_text())
            capture(mode)
            remote('goto 3;add_bookmark layout-check')
            wait_for(lambda:document()['page']==2)
            remote('goto 1;search commands;add_highlight b;close')
            expected=['single','spread','horizontal','vertical'].index(mode)+1
            wait_for(lambda:len(document()['highlights'])==expected)
            assert document()['highlights'][-1]['text'].lower().find('commands')>=0
            assert document()['highlights'][-1]['page']==1,'search/highlight mapped to the wrong PDF page'
            remote('next_page')
            expected_page=3 if mode=='spread' else 2
            wait_for(lambda:document()['page']==expected_page)
        remote('layout_horizontal;goto 4;quit');process.wait(timeout=5)
        assert process.returncode==0
        modified=library.stat().st_mtime_ns
        process=subprocess.Popen([args.binary,'--library',str(library),'--config',str(config),str(args.pdf.resolve())],stdout=log,stderr=subprocess.STDOUT)
        wait_for(lambda:library.stat().st_mtime_ns!=modified)
        assert document()['page']==3
        assert 'page_layout horizontal' in preferences.read_text()
        remote('quit');process.wait(timeout=5);assert process.returncode==0
        print('PASS: single, spread, horizontal/vertical strips, page navigation, search/highlight page mapping, saved layout and position')
    finally:
        if process.poll() is None:process.terminate();process.wait(timeout=5)
        if process.returncode!=0:
            log.seek(0);print('reader exit',process.returncode,log.read())
