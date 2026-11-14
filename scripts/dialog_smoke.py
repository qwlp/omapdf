#!/usr/bin/env python3
"""Exercise centered dialogs and nested PDF outlines with isolated state on Wayland."""
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
    def click_toc_control(offset):
        w=clients()[0];x,y=w['at'];width,height=w['size']
        card_width=min(width-48,720);card_height=min(height-30-120,254)
        px=x+(width-card_width)/2+card_width-offset
        py=y+30+(height-30-card_height)/2+22
        dispatch(f'hl.dsp.cursor.move({{x={int(px)},y={int(py)}}})');key('mouse:272');time.sleep(.25)
    def capture(path):
        w=clients()[0];x,y=w['at'];width,height=w['size']
        subprocess.run(['grim','-g',f'{x},{y} {width}x{height}',str(path)],check=True)
    try:
        wait(lambda:clients() and bool(state().get('documents')))
        address='address:'+clients()[0]['address'];dispatch('hl.dsp.focus({window='+json.dumps(address)+'})');time.sleep(.4)
        remote('goto_toc');time.sleep(.4);capture('/tmp/omapdf-dialog-contents.png')
        # Both branches start folded, without needing a manual Left key.
        key('Down');key('Return');wait(lambda:page()==2)
        remote('goto_toc');time.sleep(.3)
        click_toc_control(62)  # Expand all: two Down keys reach the nested section.
        capture('/tmp/omapdf-dialog-expanded.png')
        key('Down');key('Down');key('Return');wait(lambda:page()==1)
        remote('goto_toc');time.sleep(.3)
        click_toc_control(94)  # Collapse all: one Down key reaches Part two again.
        key('Down');key('Return');wait(lambda:page()==2)
        remote('goto_toc');time.sleep(.3)
        # Filtering reveals the folded branch and keeps both ancestor headings.
        for ch in 'routing':key(ch)
        key('Down');key('Down');key('Return');wait(lambda:page()==1)
        remote('goto_toc');time.sleep(.3);key('Right');key('Escape')
        # Clicking the dimmed page dismisses the dialog without navigating or selecting.
        remote('goto_toc');time.sleep(.3)
        w=clients()[0];x,y=w['at'];dispatch(f'hl.dsp.cursor.move({{x={x+20},y={y+120}}})');key('mouse:272');time.sleep(.3)
        capture('/tmp/omapdf-dialog-dismissed.png');assert page()==1
        remote('settings');time.sleep(.3);capture('/tmp/omapdf-dialog-settings.png')
        remote('quit');process.wait(timeout=5);assert process.returncode==0
        print('PASS: nested native outline, default folding, mouse collapse/expand all, filtered ancestor context, keyboard navigation, centered dialogs, backdrop dismissal')
    finally:
        if process.poll() is None:process.terminate();process.wait(timeout=5)
        if prior:dispatch('hl.dsp.focus({window='+json.dumps('address:'+prior)+'})')
        dispatch('hl.dsp.cursor.move({x='+str(cursor['x'])+',y='+str(cursor['y'])+'})')
