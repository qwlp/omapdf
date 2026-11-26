#!/usr/bin/env python3
"""Exercise native EPUB reflow, annotations and reading state in an isolated Wayland reader."""
import argparse,json,pathlib,subprocess,tempfile,time,os
parser=argparse.ArgumentParser();parser.add_argument('pdf',type=pathlib.Path)
parser.add_argument('--no-tools',action='store_true');parser.add_argument('--binary',default='target/release/omapdf');parser.add_argument('--screenshot',type=pathlib.Path)
args=parser.parse_args()
def hypr(*args):return subprocess.check_output(['hyprctl',*args],text=True)
def dispatch(expression):hypr('eval','hl.dispatch('+expression+')')
prior=json.loads(hypr('activewindow','-j')).get('address')
prior_cursor=json.loads(hypr('cursorpos','-j'))
with tempfile.TemporaryDirectory(prefix='omapdf-context-') as temporary:
    root=pathlib.Path(temporary);config=root/'config';config.mkdir();library=root/'library.json';log=open(root/'gui.log','w+')
    process=subprocess.Popen([args.binary,'--library',str(library),'--config',str(config),str(args.pdf.resolve())],stdout=log,stderr=subprocess.STDOUT,env=dict(os.environ,PATH='/nonexistent',XDG_CACHE_HOME=str(root/'cache')) if args.no_tools else None)
    def state():return json.loads(library.read_text())
    def document():return state()['documents'][state()['recent'][0]]
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
        address='address:'+window['address'];dispatch('hl.dsp.focus({window='+json.dumps(address)+'})');time.sleep(.7)
        remote('search Compilation;add_highlight b;add_bookmark epub-anchor;close')
        wait_for(lambda:bool(document()['highlights']))
        original_id=next(iter(state()['recent']));original=document();old_page=original['page']
        assert 'Compilation' in original['highlights'][0]['text']
        # Reflow typography and verify that text annotations and bookmarks follow their text.
        remote('set_preference epub_font_size 17')
        wait_for(lambda:next(iter(state()['recent']))!=original_id)
        wait_for(lambda:bool(document()['highlights']))
        assert 'Compilation' in document()['highlights'][0]['text']
        assert document()['page']>old_page
        assert state()['bookmarks'][-1]['location']['document']==state()['recent'][0]
        remote('layout_vertical;search Waypoints;close');time.sleep(.6)
        remote('add_bookmark epub-restored')
        wait_for(lambda:state()['bookmarks'][-1]['name']=='epub-restored')
        expected=document()['page']
        if args.screenshot:
            window=next(w for w in json.loads(hypr('clients','-j')) if w.get('pid')==process.pid)
            x,y=window['at'];w,h=window['size'];subprocess.run(['grim','-g',f'{x},{y} {w}x{h}',str(args.screenshot)],check=True)
        remote('quit');process.wait(timeout=5);assert process.returncode==0
        process=subprocess.Popen([args.binary,'--library',str(library),'--config',str(config),str(args.pdf.resolve())],stdout=log,stderr=subprocess.STDOUT,env=dict(os.environ,PATH='/nonexistent',XDG_CACHE_HOME=str(root/'cache')) if args.no_tools else None)
        time.sleep(1.5)
        assert process.poll() is None
        remote('add_bookmark reopened')
        wait_for(lambda:state()['bookmarks'][-1]['name']=='reopened')
        assert document()['page']==expected
        remote('quit');process.wait(timeout=5);assert process.returncode==0
        print('PASS: native EPUB search/selection, annotations, typography reflow, bookmark migration, continuous layout, saved position restoration, clean quit')
    finally:
        if process.poll() is None:process.terminate();process.wait(timeout=5)
        if process.returncode!=0:log.seek(0);print(log.read())
        if prior:dispatch('hl.dsp.focus({window='+json.dumps('address:'+prior)+'})')
        dispatch('hl.dsp.cursor.move({x='+str(prior_cursor['x'])+',y='+str(prior_cursor['y'])+'})')
