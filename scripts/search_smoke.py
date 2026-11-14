#!/usr/bin/env python3
"""Exercise centered search dialogs and repeated toolbar toggles in an isolated Wayland reader."""
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
    def capture(name):
        window=next(w for w in json.loads(hypr('clients','-j')) if w.get('pid')==process.pid)
        x,y=window['at'];w,h=window['size'];path=root/(name+'.png')
        subprocess.run(['grim','-g',f'{x},{y} {w}x{h}',str(path)],check=True)
        from PIL import Image
        return Image.open(path).convert('RGB')
    def click(local_x,local_y):
        window=next(w for w in json.loads(hypr('clients','-j')) if w.get('pid')==process.pid)
        x,y=window['at'];dispatch('hl.dsp.cursor.move({x='+str(x+local_x)+',y='+str(y+local_y)+'})');time.sleep(.04);key('mouse:272');time.sleep(.3)
    def dialog_bounds():
        window=next(w for w in json.loads(hypr('clients','-j')) if w.get('pid')==process.pid)
        width,height=window['size'];reader_height=height-30
        dialog_width=min(width-48,720);dialog_height=min(reader_height-120,640)
        return (width-dialog_width)//2,30+(reader_height-dialog_height)//2,dialog_width,dialog_height
    def dialog_open():
        picture=capture('state');left,top,width,height=dialog_bounds()
        samples=[max(picture.getpixel((x,top+20)))<150 for x in range(left+24,left+width-24,20)]
        return sum(samples)>len(samples)*.8
    def dialog_click(x,y):
        left,top,_,_=dialog_bounds();click(left+x,top+y)
    try:
        wait_for(lambda:library.exists() and bool(state().get('documents')) and any(w.get('pid')==process.pid for w in json.loads(hypr('clients','-j'))))
        window=next(w for w in json.loads(hypr('clients','-j')) if w.get('pid')==process.pid)
        address='address:'+window['address'];dispatch('hl.dsp.focus({window='+json.dumps(address)+'})');time.sleep(.5)
        remote('goto 1;fit_to_page_width;close');time.sleep(.5)
        # Real repeat-click tests for Search, Notes, and Settings.
        for offset in [118,86,22]:
            current=next(w for w in json.loads(hypr('clients','-j')) if w.get('pid')==process.pid)
            click(current['size'][0]-offset,48)
            try: wait_for(dialog_open)
            except AssertionError:
                capture('toolbar-timeout').save('/tmp/omapdf-search-toolbar-timeout.png')
                raise AssertionError(f'Toolbar button at offset {offset} did not open its dialog')
            current=next(w for w in json.loads(hypr('clients','-j')) if w.get('pid')==process.pid)
            click(current['size'][0]-offset,48);assert not dialog_open(),f'toolbar button {offset} did not close on repeat click'
        key('f','CTRL')
        for char in 'commands':key(char)
        time.sleep(.45)
        assert dialog_open(),'centered search dialog missing'
        if args.screenshot:
            capture('search').save(args.screenshot)
        # Commit the first live match and check its exact selection through annotation storage.
        key('Return');time.sleep(.3)
        remote('add_highlight b')
        wait_for(lambda:len(document()['highlights'])==1)
        assert document()['highlights'][-1]['text'].lower()=='commands'
        assert document()['highlights'][-1]['page']==1
        # Replace query: old live results must not override the latest query.
        key('a','CTRL')
        for char in 'portal':key(char)
        time.sleep(.4);key('Return');remote('add_highlight a')
        wait_for(lambda:len(document()['highlights'])==2)
        assert 'portal' in document()['highlights'][-1]['text'].lower()
        # Clicking a result navigates to that occurrence without clicking through into the PDF.
        dialog_click(120,214)
        remote('add_highlight g');wait_for(lambda:len(document()['highlights'])==3)
        assert 'portal' in document()['highlights'][-1]['text'].lower()
        # Whole-word matching must reject a prefix; disabling it restores results.
        dialog_click(76,107)
        key('a','CTRL')
        for char in 'porta':key(char)
        time.sleep(.4);remote('add_highlight b');time.sleep(.1)
        assert len(document()['highlights'])==3,'whole-word search matched a prefix'
        dialog_click(76,107);time.sleep(.3);key('Return');remote('add_highlight b')
        wait_for(lambda:len(document()['highlights'])==4)
        # Invalid regex clears results and selection, then recovers on the next query.
        dialog_click(125,107);key('a','CTRL');key('bracketleft');time.sleep(.4)
        remote('add_highlight b');time.sleep(.1)
        assert len(document()['highlights'])==4,'invalid regex retained a stale selection'
        key('a','CTRL')
        for char in 'portal':key(char)
        time.sleep(.4);key('Return');remote('add_highlight b')
        wait_for(lambda:len(document()['highlights'])==5)
        key('F3');remote('add_highlight a');wait_for(lambda:len(document()['highlights'])==6)
        key('F3','SHIFT');remote('add_highlight a');wait_for(lambda:len(document()['highlights'])==7)
        assert document()['highlights'][-1]['rects']==document()['highlights'][-3]['rects'],'F3/Shift+F3 did not return to the original occurrence'
        key('Escape');time.sleep(.25);assert not dialog_open(),'Escape did not close search'
        remote('quit');process.wait(timeout=5);assert process.returncode==0
        print('PASS: mouse toolbar toggles, live typing, query replacement, exact match navigation, clickable search results, whole-word matching, regex errors and recovery, Escape dismissal, clean quit')
    finally:
        if process.poll() is None:process.terminate();process.wait(timeout=5)
        if process.returncode!=0:log.seek(0);print(log.read())
        if prior:dispatch('hl.dsp.focus({window='+json.dumps('address:'+prior)+'})')
        dispatch('hl.dsp.cursor.move({x='+str(prior_cursor['x'])+',y='+str(prior_cursor['y'])+'})')
