#!/usr/bin/env python3
"""Exercise command completion and execution with isolated state on Wayland."""
import argparse,json,pathlib,subprocess,tempfile,time
parser=argparse.ArgumentParser();parser.add_argument('--binary',default='target/debug/omapdf');args=parser.parse_args()
binary=str(pathlib.Path(args.binary).resolve())
def hypr(*args):return subprocess.check_output(['hyprctl',*args],text=True)
def dispatch(expr):return hypr('eval','hl.dispatch('+expr+')')
prior=json.loads(hypr('activewindow','-j')).get('address')
cursor=json.loads(hypr('cursorpos','-j'))
with tempfile.TemporaryDirectory(prefix='omapdf-dialog-') as temporary:
    root=pathlib.Path(temporary);pdf=root/'nested.pdf';library=root/'library.json'
    objects=[
        '<< /Type /Catalog /Pages 2 0 R /Outlines 6 0 R >>',
        '<< /Type /Pages /Kids [3 0 R 4 0 R 5 0 R] /Count 3 >>',
        *['<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << >> >>']*3,
        '<< /Type /Outlines /First 7 0 R /Last 10 0 R /Count 4 >>',
        '<< /Title (Part one) /Parent 6 0 R /First 8 0 R /Last 8 0 R /Next 10 0 R /Count 2 >>',
        '<< /Title (Chapter one) /Parent 7 0 R /Dest [3 0 R /Fit] /First 9 0 R /Last 9 0 R /Count 1 >>',
        '<< /Title (Routing details) /Parent 8 0 R /Dest [4 0 R /Fit] >>',
        '<< /Title (Part two) /Parent 6 0 R /Prev 7 0 R /Dest [5 0 R /Fit] >>',
    ]
    data='%PDF-1.7\n';offsets=[]
    for i,obj in enumerate(objects):offsets.append(len(data));data+=f'{i+1} 0 obj\n{obj}\nendobj\n'
    xref=len(data);data+=f'xref\n0 {len(objects)+1}\n0000000000 65535 f \n'
    for offset in offsets:data+=f'{offset:010} 00000 n \n'
    data+=f'trailer\n<< /Size {len(objects)+1} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n';pdf.write_text(data)
    base=[binary,'--library',str(library),'--config',str(root/'config')]
    log=open(root/'log','w+');process=subprocess.Popen(base+[str(pdf)],stdout=log,stderr=subprocess.STDOUT)
    def clients():return [w for w in json.loads(hypr('clients','-j')) if w.get('pid')==process.pid]
    def state():return json.loads(library.read_text()) if library.exists() else {}
    def page():return next(iter(state()['documents'].values()))['page']
    def wait(test):
        end=time.monotonic()+15
        while time.monotonic()<end:
            if process.poll() is not None:log.seek(0);raise AssertionError(f'exit={process.returncode}: '+log.read())
            if test():return
            time.sleep(.1)
        log.seek(0);raise AssertionError('timeout: '+log.read())
    def remote(command):subprocess.run(base+['--execute-command',command],check=True,timeout=5)
    def key(name,mods=''):
        dispatch('hl.dsp.send_shortcut({window='+json.dumps(address)+',mods='+json.dumps(mods)+',key='+json.dumps(name)+'})');time.sleep(.1)
    def capture(path):
        w=clients()[0];x,y=w['at'];width,height=w['size']
        subprocess.run(['grim','-g',f'{x},{y} {width}x{height}',str(path)],check=True)
    def type_text(text):
        for char in text:
            key('minus','SHIFT') if char == '_' else key({' ':'space','.':'period',';':'semicolon'}.get(char,char))
    def command(text):
        remote('close;command');time.sleep(.3);type_text(text)
    try:
        wait(lambda:clients() and bool(state().get('documents')))
        address='address:'+clients()[0]['address'];dispatch('hl.dsp.focus({window='+json.dumps(address)+'})');time.sleep(.3)
        command('got');capture('/tmp/omapdf-command-completion.png');key('Tab');type_text('2');key('Return');time.sleep(.3);capture('/tmp/omapdf-command-executed.png')
        wait(lambda:page()==1)
        command('set_preference wheel_s');key('Tab');type_text('1.7');key('Return')
        prefs=root/'config/prefs_user.config'
        wait(lambda:prefs.exists() and 'wheel_scroll_multiplier 1.7' in prefs.read_text())
        command('set_preference smooth_scroll ');key('Down');key('Return')
        wait(lambda:'smooth_scroll 0' in prefs.read_text())
        command('layout_s');key('Tab');key('Tab');key('Tab','SHIFT');key('Return')
        wait(lambda:'page_layout single' in prefs.read_text())
        command('layout_ver');key('Right');key('Return')
        wait(lambda:'page_layout vertical' in prefs.read_text())
        command('layout_');key('Down');key('Down');key('Tab');key('Return')
        wait(lambda:'page_layout single' in prefs.read_text())
        command('layout_sp')
        w=clients()[0];x,y=w['at'];width,height=w['size']
        shot=root/'mouse.png';capture(shot)
        from PIL import Image
        image=Image.open(shot).convert('RGB')
        top=next(row for row in range(max(70,height-400),height-24) if max(image.getpixel((width//2,row)))<130)
        dispatch(f'hl.dsp.cursor.move({{x={x+width//2},y={y+top+24}}})');key('mouse:272');key('Return')
        wait(lambda:'page_layout spread' in prefs.read_text())
        # Exact command input opens an argument hint instead of running an incomplete goto.
        command('goto');key('Return');type_text('3');key('Return');wait(lambda:page()==2)
        # Starting a new prompt surfaces commands from this session.
        command('');capture('/tmp/omapdf-command-history.png');key('Escape')
        remote('quit');process.wait(timeout=5);assert process.returncode==0
        print('PASS: typed command completion, argument hints, preference names and values, Tab cycling, Shift+Tab reversal, arrow selection, right-arrow completion, mouse completion, and command execution')
    finally:
        if process.poll() is None:process.terminate();process.wait(timeout=5)
        if prior:dispatch('hl.dsp.focus({window='+json.dumps('address:'+prior)+'})')
        dispatch('hl.dsp.cursor.move({x='+str(cursor['x'])+',y='+str(cursor['y'])+'})')
