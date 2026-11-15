#!/usr/bin/env python3
"""Deliver real Wayland wheel events; ensure one animated distance per notch, not a native jump plus animation."""
import argparse,json,pathlib,select,subprocess,tempfile,time
parser=argparse.ArgumentParser();parser.add_argument('--binary',default='target/debug/omapdf');parser.add_argument('--classic',action='store_true');parser.add_argument('--preview',action='store_true');args=parser.parse_args()
def hypr(*args):return subprocess.check_output(['hyprctl',*args],text=True)
def dispatch(expr):return hypr('eval','hl.dispatch('+expr+')')
prior=json.loads(hypr('activewindow','-j')).get('address');cursor=json.loads(hypr('cursorpos','-j'))
with tempfile.TemporaryDirectory(prefix='omapdf-wheel-') as temporary:
    root=pathlib.Path(temporary);config=root/'config';config.mkdir();library=root/'library.json'
    (config/'prefs_user.config').write_text('page_layout vertical\nmax_page_width 800\nsmooth_scroll 1\nwheel_scroll_glide '+str(int(args.classic))+'\n')
    xml=next((pathlib.Path.home()/'.cargo/registry/src').glob('*/wayland-protocols-wlr-*/wlr-protocols/unstable/wlr-virtual-pointer-unstable-v1.xml'))
    subprocess.run(['wayland-scanner','client-header',str(xml),str(root/'virtual-pointer-client.h')],check=True)
    subprocess.run(['wayland-scanner','private-code',str(xml),str(root/'virtual-pointer.c')],check=True)
    flags=subprocess.check_output(['pkg-config','--cflags','--libs','wayland-client'],text=True).split()
    subprocess.run(['cc','-Wall','-Wextra','-Werror','-I'+str(root),'scripts/wheel_input.c',str(root/'virtual-pointer.c'),'-o',str(root/'wheel-input'),*flags],check=True)
    # A tall single page keeps local bookmark coordinates stable throughout the
    # faster wheel burst; a short multi-page fixture changes the reference page.
    objects=['<< /Type /Catalog /Pages 2 0 R >>','<< /Type /Pages /Kids [3 0 R] /Count 1 >>','<< /Type /Page /Parent 2 0 R /MediaBox [0 0 600 2400] /Resources << >> >>']
    data='%PDF-1.7\n';offsets=[]
    for i,obj in enumerate(objects):offsets.append(len(data));data+=f'{i+1} 0 obj\n{obj}\nendobj\n'
    xref=len(data);data+='xref\n0 4\n0000000000 65535 f \n'
    for offset in offsets:data+=f'{offset:010} 00000 n \n'
    data+=f'trailer\n<< /Size 4 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n'
    pdf=root/'wheel.pdf';pdf.write_text(data)
    base=[args.binary,'--library',str(library),'--config',str(config)]
    log=open(root/'gui.log','w+');process=subprocess.Popen(base+[str(pdf)],stdout=log,stderr=subprocess.STDOUT);pointer=None
    def clients():return [w for w in json.loads(hypr('clients','-j')) if w.get('pid')==process.pid]
    def state():return json.loads(library.read_text()) if library.exists() else {}
    def wait(test):
        end=time.monotonic()+15
        while time.monotonic()<end:
            if process.poll() is not None:log.seek(0);raise AssertionError(log.read())
            if test():return
            time.sleep(.05)
        log.seek(0);raise AssertionError('timeout: '+log.read())
    def remote(command):subprocess.run(base+['--execute-command',command],check=True,timeout=5)
    def response():
        assert select.select([pointer.stdout],[],[],5)[0], 'virtual pointer timed out'
        return pointer.stdout.readline().strip()
    def wheel(steps):
        pointer.stdin.write(str(steps)+'\n');pointer.stdin.flush();assert response()=='OK'
    def position(name):
        remote('add_bookmark '+name)
        wait(lambda:any(b['name']==name for b in state().get('bookmarks',[])))
        return next(b['location']['offset'][1] for b in state()['bookmarks'] if b['name']==name)
    try:
        wait(lambda:clients() and bool(state().get('documents')))
        pointer=subprocess.Popen([str(root/'wheel-input')],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True)
        assert response()=='READY'
        w=clients()[0];address='address:'+w['address'];dispatch('hl.dsp.focus({window='+json.dumps(address)+'})')
        x,y=w['at'];width,height=w['size'];dispatch(f'hl.dsp.cursor.move({{x={x+width//2},y={y+height//2}}})')
        remote('goto 1;fit_to_page_width;goto_top_of_page');time.sleep(.6)
        if args.preview:
            remote('overview 1');time.sleep(.4)
        start=position('start');wheel(1);time.sleep(.55);single=position('single')
        assert abs((single-start)-(-120/800))<.003, f'wheel handled more than once: {start=} {single=}'
        for _ in range(8):wheel(1);time.sleep(.125)
        time.sleep(.55);burst=position('burst')
        assert abs((burst-single)-(-960/800))<.004, f'burst distance mismatch: {single=} {burst=}'
        wheel(-1);time.sleep(.55);reverse=position('reverse')
        assert abs((reverse-burst)-(120/800))<.003, 'wheel reversal distance mismatch'
        remote('quit');process.wait(timeout=5);assert process.returncode==0
        print('PASS: real Wayland wheel events, exactly one distance per notch (including with preview when requested), repeated inputs, reversal, clean quit')
    finally:
        if pointer is not None:pointer.stdin.close();pointer.wait(timeout=5)
        if process.poll() is None:process.terminate();process.wait(timeout=5)
        dispatch('hl.dsp.cursor.move({x='+str(cursor['x'])+',y='+str(cursor['y'])+'})')
        if prior:dispatch('hl.dsp.focus({window='+json.dumps('address:'+prior)+'})')
