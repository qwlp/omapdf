#!/usr/bin/env python3
"""Verify document palettes, live theme refresh and rounded/square dialog corners on Wayland."""
import argparse,io,json,os,pathlib,subprocess,tempfile,time
from PIL import Image
parser=argparse.ArgumentParser();parser.add_argument('--binary',default='target/debug/omapdf');args=parser.parse_args()
def hypr(*args):return subprocess.check_output(['hyprctl',*args],text=True)
def dispatch(expr):return hypr('eval','hl.dispatch('+expr+')')
prior=json.loads(hypr('activewindow','-j')).get('address');cursor=json.loads(hypr('cursorpos','-j'))
with tempfile.TemporaryDirectory(prefix='omapdf-theme-') as temporary:
    root=pathlib.Path(temporary);theme=root/'state/omarchy/current/theme/colors.toml';theme.parent.mkdir(parents=True)
    env=dict(os.environ,XDG_STATE_HOME=str(root/'state'))
    def palette(bg,fg):theme.write_text(f'background = "#101c2c"\ndark_background = "#{bg}"\nforeground = "#{fg}"\naccent = "#a090f0"\nlighter_background = "#20304c"\n')
    for index,book in enumerate(['examples/reading-lab.pdf','examples/reading-lab.epub']):
        palette('081020','e0c0a0');config=root/f'config-{index}';config.mkdir();library=root/f'library-{index}.json'
        (config/'prefs_user.config').write_text('document_theme 1\nrounded_corners 1\nmax_page_width 600\n')
        base=[args.binary,'--library',str(library),'--config',str(config)];log=open(root/f'log-{index}','w+')
        process=subprocess.Popen(base+[str(pathlib.Path(book).resolve())],env=env,stdout=log,stderr=subprocess.STDOUT)
        def clients():return [w for w in json.loads(hypr('clients','-j')) if w.get('pid')==process.pid]
        def wait(test):
            end=time.monotonic()+15
            while time.monotonic()<end:
                if process.poll() is not None:log.seek(0);raise AssertionError(log.read())
                if test():return
                time.sleep(.1)
            log.seek(0);raise AssertionError('timeout: '+log.read())
        def remote(command):subprocess.run(base+['--execute-command',command],env=env,check=True,timeout=5)
        def capture():
            w=clients()[0];x,y=w['at'];width,height=w['size']
            return Image.open(io.BytesIO(subprocess.check_output(['grim','-g',f'{x},{y} {width}x{height}','-']))).convert('RGB')
        def paper(color):
            im=capture();p=im.getpixel((im.width//2+280,120));return max(abs(p[i]-color[i]) for i in range(3))<5
        def themed_ink(color):
            im=capture();crop=im.crop((max(0,im.width//2-300),90,min(im.width,im.width//2+300),im.height-45))
            return sum(count for count,rgb in (crop.getcolors(crop.width*crop.height) or []) if max(abs(rgb[i]-color[i]) for i in range(3))<8)>10
        try:
            wait(lambda:clients() and library.exists() and bool(json.loads(library.read_text()).get('documents')))
            address='address:'+clients()[0]['address'];dispatch('hl.dsp.focus({window='+json.dumps(address)+'})');time.sleep(.4)
            if index:remote('goto 2')
            wait(lambda:paper((8,16,32)) and themed_ink((224,192,160)))
            capture().save(f'/tmp/omapdf-themed-{index}.png')
            remote('toggle_dark_mode');wait(lambda:paper((255,255,255)))
            # A preference saved by another tab/window must update this reader too.
            preferences=config/'prefs_user.config'
            preferences.write_text(preferences.read_text().replace('document_theme 0','document_theme 1'))
            wait(lambda:paper((8,16,32)))
            palette('10220a','c0d0a0');wait(lambda:paper((16,34,10)) and themed_ink((192,208,160)))
            if index==0:
                remote('set_preference document_theme 0;settings');time.sleep(.4)
                rounded=capture();width,height=rounded.size
                card_width=min(width-48,720);card_height=min(height-120,640)
                left=int((width-card_width)/2);top=int(30+(height-30-card_height)/2)
                corner=rounded.getpixel((left+2,top+2))
                remote('set_preference rounded_corners 0');time.sleep(.3);square=capture().getpixel((left+2,top+2))
                assert sum(abs(corner[i]-square[i]) for i in range(3))>20, f'rounded corner was overwritten by a square child: {corner=} {square=}'
                assert max(abs(square[i]-(16,28,44)[i]) for i in range(3))<8, square
                remote('set_preference rounded_corners 1');time.sleep(.3);capture().save('/tmp/omapdf-clean-rounded-corners.png')
            remote('quit');process.wait(timeout=5);assert process.returncode==0
        finally:
            if process.poll() is None:process.terminate();process.wait(timeout=5)
            dispatch('hl.dsp.cursor.move({x='+str(cursor['x'])+',y='+str(cursor['y'])+'})')
            if prior:dispatch('hl.dsp.focus({window='+json.dumps('address:'+prior)+'})')
    dispatch('hl.dsp.cursor.move({x='+str(cursor['x'])+',y='+str(cursor['y'])+'})')
    if prior:dispatch('hl.dsp.focus({window='+json.dumps('address:'+prior)+'})')
print('PASS: PDF/EPUB page and ink palettes, reversible theme toggle, live palette/preference refresh, rounded/square corners')
