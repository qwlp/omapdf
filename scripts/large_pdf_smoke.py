#!/usr/bin/env python3
"""Exercise fast first-page display and background indexing of the large PDF in an isolated Wayland reader."""
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
    started=time.monotonic()
    process=subprocess.Popen([args.binary,'--library',str(library),'--config',str(config),str(args.pdf.resolve())],stdout=log,stderr=subprocess.STDOUT)
    def state():return json.loads(library.read_text())
    def document():return next(iter(state()['documents'].values()))
    def remote(command):subprocess.run([args.binary,'--library',str(library),'--execute-command',command],check=True,timeout=5)
    def key(value,mods=''):dispatch('hl.dsp.send_shortcut({window='+json.dumps(address)+',mods='+json.dumps(mods)+',key='+json.dumps(value)+'})');time.sleep(.06)
    def wait_for(test):
        end=time.monotonic()+45
        while time.monotonic()<end:
            if process.poll() is not None:
                log.seek(0);raise AssertionError('reader exited with code '+str(process.returncode)+': '+log.read())
            if test():return
            time.sleep(.05)
        try:capture()
        except Exception:pass
        log.seek(0);raise AssertionError('timeout; last state='+str(state())+'; '+log.read())
    def capture():
        window=next(w for w in json.loads(hypr('clients','-j')) if w.get('pid')==process.pid)
        x,y=window['at'];w,h=window['size'];path=root/'first-page.png'
        subprocess.run(['grim','-g',f'{x},{y} {w}x{h}',str(path)],check=True)
        from PIL import Image
        picture=Image.open(path).convert('RGB')
        colors=[picture.getpixel((col,row)) for row in range(60,h-60,5) for col in range(100,w-100,5)]
        if args.screenshot:
            import shutil;shutil.copyfile(path,args.screenshot)
        white=sum(min(color)>245 for color in colors)/len(colors)
        ink=sum(min(color)<220 for color in colors)/len(colors)
        saturated=sum(max(color)-min(color)>60 for color in colors)/len(colors)
        return (white>.3 or saturated>.2) and ink>.002,path
    try:
        wait_for(lambda:library.exists() and bool(state().get('documents')))
        metadata_time=time.monotonic()-started
        window=next(w for w in json.loads(hypr('clients','-j')) if w.get('pid')==process.pid)
        address='address:'+window['address'];dispatch('hl.dsp.focus({window='+json.dumps(address)+'})')
        deadline=started+6
        while time.monotonic()<deadline:
            drawn,path=capture()
            if drawn:break
            time.sleep(.08)
        assert drawn,'first page did not become visible within six seconds'
        first_page_time=time.monotonic()-started
        print(f'First page: metadata {metadata_time:.2f}s, drawn {first_page_time:.2f}s',flush=True)
        if prior:dispatch('hl.dsp.focus({window='+json.dumps('address:'+prior)+'})')
        if args.screenshot:
            import shutil;shutil.copyfile(path,args.screenshot)
        assert metadata_time<4,'document opening still blocked on all-page indexing'
        # Jump immediately to the end while the background index is still running.
        remote('goto_end;add_bookmark far-page')
        wait_for(lambda:bool(state()['bookmarks']))
        assert document()['page']==1219
        print('Last page navigation passed',flush=True)
        remote('goto 1;search permittivity;add_highlight b;close;add_bookmark search-completed')
        wait_for(lambda:bool(state()['bookmarks']) and state()['bookmarks'][-1]['name']=='search-completed')
        assert document()['highlights'],'completed search did not select a match'
        assert 'permittivity' in document()['highlights'][-1]['text'].lower()
        remote('quit');process.wait(timeout=5);assert process.returncode==0
        print(f'PASS: metadata {metadata_time:.2f}s; first drawn page {first_page_time:.2f}s; immediate last-page navigation; full-document background search and annotation; clean quit')
    finally:
        if process.poll() is None:process.terminate();process.wait(timeout=5)
        if process.returncode!=0:log.seek(0);print(log.read())
        if prior:dispatch('hl.dsp.focus({window='+json.dumps('address:'+prior)+'})')
        dispatch('hl.dsp.cursor.move({x='+str(prior_cursor['x'])+',y='+str(prior_cursor['y'])+'})')
