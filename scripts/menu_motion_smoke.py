#!/usr/bin/env python3
"""Check intermediate menu scroll frames, wheel reversal, and transient notification layout."""
import argparse,json,pathlib,select,subprocess,tempfile,time
parser=argparse.ArgumentParser();parser.add_argument('--binary',default='target/debug/omapdf');args=parser.parse_args()
def hypr(*args):return subprocess.check_output(['hyprctl',*args],text=True)
def dispatch(expr):return hypr('eval','hl.dispatch('+expr+')')
prior=json.loads(hypr('activewindow','-j')).get('address');cursor=json.loads(hypr('cursorpos','-j'))
with tempfile.TemporaryDirectory(prefix='omapdf-wheel-') as temporary:
    root=pathlib.Path(temporary);config=root/'config';config.mkdir();library=root/'library.json'
    (config/'prefs_user.config').write_text('page_layout vertical\nmax_page_width 800\nsmooth_scroll 1\n')
    xml=next((pathlib.Path.home()/'.cargo/registry/src').glob('*/wayland-protocols-wlr-*/wlr-protocols/unstable/wlr-virtual-pointer-unstable-v1.xml'))
    subprocess.run(['wayland-scanner','client-header',str(xml),str(root/'virtual-pointer-client.h')],check=True)
    subprocess.run(['wayland-scanner','private-code',str(xml),str(root/'virtual-pointer.c')],check=True)
    flags=subprocess.check_output(['pkg-config','--cflags','--libs','wayland-client'],text=True).split()
    subprocess.run(['cc','-Wall','-Wextra','-Werror','-I'+str(root),'scripts/wheel_input.c',str(root/'virtual-pointer.c'),'-o',str(root/'wheel-input'),*flags],check=True)
    base=[args.binary,'--library',str(library),'--config',str(config)]
    log=open(root/'gui.log','w+');process=subprocess.Popen(base+[str(pathlib.Path("examples/reading-lab.epub").resolve())],stdout=log,stderr=subprocess.STDOUT);pointer=None
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
    def capture():
        import io
        from PIL import Image
        w=clients()[0];x,y=w['at'];width,height=w['size']
        return Image.open(io.BytesIO(subprocess.check_output(['grim','-g',f'{x},{y} {width}x{height}','-']))).convert('RGB')
    try:
        wait(lambda:clients() and bool(state().get('documents')))
        pointer=subprocess.Popen([str(root/'wheel-input')],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True)
        assert response()=='READY'
        w=clients()[0];address='address:'+w['address'];dispatch('hl.dsp.focus({window='+json.dumps(address)+'})')
        x,y=w['at'];width,height=w['size'];dispatch(f'hl.dsp.cursor.move({{x={x+width//2},y={y+height//2}}})')
        time.sleep(.5)
        for name,command in [('settings','settings'),('contents','goto_toc;expand_toc'),('search','search Reading')]:
            remote('close;'+command);time.sleep(1.8)
            before=capture();box=(max(0,width//2-340),height//2-130,min(width,width//2+340),height//2+130)
            start=before.crop(box).tobytes();wheel(1)
            frames=[]
            for _ in range(6):
                frames.append(capture().crop(box).tobytes());time.sleep(.025)
            time.sleep(.7);settled=capture();end=settled.crop(box).tobytes()
            assert end!=start, f'{name} did not scroll'
            print(name, 'intermediate frames:',len(set(frames)-{start,end}),flush=True)
            # The crisp 150 ms animation may finish during the second screenshot.
            assert len(set(frames)-{start,end})>=1, f'{name} jumped without intermediate frames'
            settled.save('/tmp/omapdf-smooth-'+name+'.png')
            wheel(-1);time.sleep(.8)
            assert capture().crop(box).tobytes()==start, f'{name} reversal did not return to original position'
        remote('close');time.sleep(.5)
        before=capture();remote('set_status_string Copied selection');time.sleep(.25);shown=capture()
        shown.save('/tmp/omapdf-notification-pill.png')
        box=(width//2-180,height-80,width//2+180,height-10)
        assert before.crop(box).tobytes()!=shown.crop(box).tobytes(), 'notification missing'
        assert before.crop((0,height-50,100,height)).tobytes()==shown.crop((0,height-50,100,height)).tobytes(), 'notification created a full-width bar'
        time.sleep(3.2)
        assert capture().crop(box).tobytes()==before.crop(box).tobytes(), 'notification did not dismiss'
        remote('command');time.sleep(.3);capture().save('/tmp/omapdf-command-pill.png');remote('close')
        remote('quit');process.wait(timeout=5);assert process.returncode==0
        print('PASS: animated settings, contents and search scrolling, wheel reversal, compact notification and automatic dismissal')
    finally:
        if pointer is not None:pointer.stdin.close();pointer.wait(timeout=5)
        if process.poll() is None:process.terminate();process.wait(timeout=5)
        dispatch('hl.dsp.cursor.move({x='+str(cursor['x'])+',y='+str(cursor['y'])+'})')
        if prior:dispatch('hl.dsp.focus({window='+json.dumps('address:'+prior)+'})')
