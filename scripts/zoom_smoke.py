#!/usr/bin/env python3
"""Exercise zoom anchoring beneath the real Wayland cursor in an isolated Wayland reader."""
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
        x,y=window['at'];width,height=window['size']
        remote('goto 1;layout_single;fit_to_page_width;close');time.sleep(.6)
        initial=min(width-32,1200);center=(width-initial)/2
        pointer_x=center+initial*.7;pointer_y=66+16+min(550,(height-90)*.5)
        logical=[(pointer_x-center)/initial,(pointer_y-66-16)/initial]
        dispatch('hl.dsp.cursor.move({x='+str(x+int(pointer_x))+',y='+str(y+int(pointer_y))+'})');time.sleep(.1)
        # Capture the actual initial scroll position rather than assuming it is zero.
        remote('add_bookmark zoom-baseline')
        wait_for(lambda:bool(state()['bookmarks']) and state()['bookmarks'][-1]['name']=='zoom-baseline')
        baseline=state()['bookmarks'][-1]['location']
        logical=[(int(pointer_x)-center)/initial-baseline['offset'][0],(int(pointer_y)-82)/initial-baseline['offset'][1]]
        for index,key_name in enumerate(['equal','equal','equal','minus','minus']):
            key(key_name);time.sleep(.35)
            remote('add_bookmark zoom-'+str(index))
            wait_for(lambda:bool(state()['bookmarks']) and state()['bookmarks'][-1]['name']=='zoom-'+str(index))
            location=state()['bookmarks'][-1]['location'];new_width=800*location['zoom']
            new_center=(max(width,new_width+32)-new_width)/2
            predicted=[new_center+(logical[0]+location['offset'][0])*new_width,82+(logical[1]+location['offset'][1])*new_width]
            assert abs(predicted[0]-int(pointer_x))<2,(key_name,predicted,pointer_x,location)
            assert abs(predicted[1]-int(pointer_y))<2,(key_name,predicted,pointer_y,location)
        remote('quit');process.wait(timeout=5);assert process.returncode==0
        print('PASS: repeated zoom-in/out preserves the point under the real Wayland cursor and starts from displayed fit width')
    finally:
        if process.poll() is None:process.terminate();process.wait(timeout=5)
        if process.returncode!=0:log.seek(0);print(log.read())
        if prior:dispatch('hl.dsp.focus({window='+json.dumps('address:'+prior)+'})')
        dispatch('hl.dsp.cursor.move({x='+str(prior_cursor['x'])+',y='+str(prior_cursor['y'])+'})')
