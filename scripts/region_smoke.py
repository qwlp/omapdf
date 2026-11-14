#!/usr/bin/env python3
"""Exercise native rectangle/image capture with real Wayland drags and clipboard requests."""
import argparse,io,json,os,pathlib,subprocess,tempfile,time
from PIL import Image
parser=argparse.ArgumentParser();parser.add_argument('--binary',default='target/release/omapdf');parser.add_argument('--no-tools',action='store_true');args=parser.parse_args()
def hypr(*arguments):return subprocess.check_output(['hyprctl',*arguments],text=True)
def dispatch(expression):return hypr('eval','hl.dispatch('+expression+')')
def pdf_bytes():
    streams=['1 0 0 rg 0 0 150 200 re f\n0 1 0 rg 150 0 150 200 re f\n','0 0 1 rg 0 0 300 200 re f\n']
    objects=['<< /Type /Catalog /Pages 2 0 R >>','<< /Type /Pages /Kids [3 0 R 5 0 R] /Count 2 >>',
        '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Contents 4 0 R >>',f'<< /Length {len(streams[0])} >>\nstream\n{streams[0]}endstream',
        '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Contents 6 0 R >>',f'<< /Length {len(streams[1])} >>\nstream\n{streams[1]}endstream']
    data='%PDF-1.4\n';offsets=[]
    for index,obj in enumerate(objects):offsets.append(len(data));data+=f'{index+1} 0 obj\n{obj}\nendobj\n'
    xref=len(data);data+=f'xref\n0 {len(objects)+1}\n0000000000 65535 f \n'
    for offset in offsets:data+=f'{offset:010} 00000 n \n'
    data+=f'trailer\n<< /Size {len(objects)+1} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n';return data.encode()
def noise_pdf():
    import random
    pixels=random.Random(7).randbytes(600*400*3).hex()+'>'
    stream='q 300 0 0 200 0 0 cm /Im0 Do Q\nBT /F1 18 Tf 30 150 Td (Clipboard research text) Tj ET\n'
    objects=['<< /Type /Catalog /Pages 2 0 R >>','<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
        '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Resources << /XObject << /Im0 5 0 R >> /Font << /F1 6 0 R >> >> /Contents 4 0 R >>',
        f'<< /Length {len(stream)} >>\nstream\n{stream}endstream',
        f'<< /Type /XObject /Subtype /Image /Width 600 /Height 400 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /ASCIIHexDecode /Length {len(pixels)} >>\nstream\n{pixels}\nendstream',
        '<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>']
    data='%PDF-1.4\n';offsets=[]
    for index,obj in enumerate(objects):offsets.append(len(data));data+=f'{index+1} 0 obj\n{obj}\nendobj\n'
    xref=len(data);data+=f'xref\n0 {len(objects)+1}\n0000000000 65535 f \n'
    for offset in offsets:data+=f'{offset:010} 00000 n \n'
    data+=f'trailer\n<< /Size {len(objects)+1} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n';return data.encode()
prior=json.loads(hypr('activewindow','-j')).get('address');cursor=json.loads(hypr('cursorpos','-j'))
prior_type=None;prior_clip=None
try:
    types=subprocess.check_output(['wl-paste','--list-types'],timeout=3,text=True,stderr=subprocess.DEVNULL).splitlines()
    prior_type=next((t for t in ['image/png','text/plain;charset=utf-8','text/plain'] if t in types),next(iter(types),None))
    if prior_type:prior_clip=subprocess.check_output(['wl-paste','--no-newline','--type',prior_type],timeout=3)
except (subprocess.SubprocessError,OSError):pass
with tempfile.TemporaryDirectory(prefix='omapdf-region-') as temporary:
    root=pathlib.Path(temporary);config=root/'config';config.mkdir();library=root/'library.json';log=open(root/'reader.log','w+')
    (config/'prefs_user.config').write_text('ui_animations 0\nmax_page_width 600\npage_padding 16\n')
    pdf=root/'image-only.pdf';pdf.write_bytes(pdf_bytes());original=pdf.read_bytes()
    base=[args.binary,'--library',str(library),'--config',str(config)]
    environment=dict(os.environ,PATH='/nonexistent',XDG_CACHE_HOME=str(root/'cache')) if args.no_tools else None
    process=subprocess.Popen(base+[str(pdf)],stdout=log,stderr=subprocess.STDOUT,env=environment);address=None;held=False;consumer=None
    def state():return json.loads(library.read_text()) if library.exists() else {}
    def wait(test):
        deadline=time.monotonic()+25
        while time.monotonic()<deadline:
            if process.poll() is not None:log.seek(0);raise AssertionError(log.read())
            if test():return
            time.sleep(.05)
        log.seek(0);raise AssertionError('timeout: '+log.read())
    def remote(command):subprocess.run(base+['--execute-command',command],check=True,timeout=5);time.sleep(.25)
    def key(key,mods=''):dispatch('hl.dsp.send_shortcut({window='+json.dumps(address)+',mods='+json.dumps(mods)+',key='+json.dumps(key)+'})');time.sleep(.08)
    def mouse_state(value):dispatch('hl.dsp.send_key_state({window='+json.dumps(address)+',mods="",key="mouse:272",state='+json.dumps(value)+'})');time.sleep(.08)
    def window():return next(w for w in json.loads(hypr('clients','-j')) if w.get('pid')==process.pid)
    def picture():
        w=window();x,y=w['at'];width,height=w['size'];return Image.open(io.BytesIO(subprocess.check_output(['grim','-g',f'{x},{y} {width}x{height}','-']))).convert('RGB')
    def color_bounds(image,colors):
        mask=Image.new('1',image.size);mask.putdata([pixel in colors for pixel in image.get_flattened_data()]);return mask.getbbox()
    def drag(a,b,image):
        global held
        w=window();density=image.width/w['size'][0]
        def move(p):dispatch('hl.dsp.cursor.move({x='+str(round(w['at'][0]+p[0]/density))+',y='+str(round(w['at'][1]+p[1]/density))+'})');time.sleep(.08)
        move(a);mouse_state('down');held=True
        for step in range(1,7):move((a[0]+(b[0]-a[0])*step/6,a[1]+(b[1]-a[1])*step/6))
        mouse_state('up');held=False;time.sleep(.25)
    def marker(name):return any(b['name']==name for b in state().get('bookmarks',[]))
    try:
        wait(lambda:bool(state().get('documents')) and any(w.get('pid')==process.pid for w in json.loads(hypr('clients','-j'))))
        address='address:'+window()['address'];dispatch('hl.dsp.focus({window='+json.dumps(address)+'})');time.sleep(.8)
        for index,layout in enumerate(['single','spread','horizontal','vertical']):
            key('Escape')
            remote('close;layout_'+layout+';goto 1;fit_to_page_width');time.sleep(.8)
            image=picture();colors={(255,0,0),(0,255,0)}
            if layout!='single':colors.add((0,0,255))
            bounds=color_bounds(image,colors);assert bounds,(layout,'no native page pixels')
            left,top,right,bottom=bounds;a=(left+15,top+15);b=(right-15,bottom-15)
            output=root/(layout+'.png');name='exported-'+layout
            command=('select_rect;export_image ' if layout=='spread' else 'export_image ')+str(output)+';add_bookmark '+name
            remote(command);assert not marker(name),'continuation ran before the drag'
            if index%2: a,b=b,a
            drag(a,b,image);wait(lambda:marker(name))
            assert output.is_file(),(layout,'missing PNG')
            exported=Image.open(output).convert('RGB');counts=dict((color,count) for count,color in exported.getcolors(exported.width*exported.height))
            assert counts.get((255,0,0),0)>100 and counts.get((0,255,0),0)>100,(layout,counts)
            if layout!='single':assert counts.get((0,0,255),0)>100,(layout,'cross-page image missing the second page')
            assert abs(exported.width-abs(a[0]-b[0]))<=2 and abs(exported.height-abs(a[1]-b[1]))<=2,(layout,exported.size,a,b)
            remote('copy_image;add_bookmark copied-'+layout);wait(lambda:marker('copied-'+layout))
            clipboard=Image.open(io.BytesIO(subprocess.check_output(['wl-paste','--no-newline','--type','image/png'],timeout=5))).convert('RGB')
            assert clipboard.size==exported.size and clipboard.tobytes()==exported.tobytes(),(layout,'clipboard differs from native PNG')
            saved=output.read_bytes();remote('export_image '+str(output)+';add_bookmark overwrite-checked-'+layout)
            wait(lambda:marker('overwrite-checked-'+layout));assert output.read_bytes()==saved
        # Selection requests and filename prompts must discard their continuations on Escape.
        remote('select_rect;add_bookmark cancelled-region');key('Escape');remote('add_bookmark after-region-cancel');wait(lambda:marker('after-region-cancel'))
        assert not marker('cancelled-region')
        remote('export_image;add_bookmark cancelled-image-path');key('Escape');remote('add_bookmark after-path-cancel');wait(lambda:marker('after-path-cancel'))
        assert not marker('cancelled-image-path')
        # The original clipboard command requests its own rectangle when none is selected.
        remote('copy_screenshot_to_clipboard;add_bookmark clipboard-drag')
        time.sleep(.4);image=picture();bounds=color_bounds(image,{(255,0,0),(0,255,0)});left,top,right,bottom=bounds
        drag((left+20,top+20),(right-20,bottom-20),image);wait(lambda:marker('clipboard-drag'))
        copied=Image.open(io.BytesIO(subprocess.check_output(['wl-paste','--no-newline','--type','image/png'],timeout=5)))
        assert copied.width>20 and copied.height>20
        assert pdf.read_bytes()==original
        # A slow recipient must not block the native Wayland event loop on a large PNG.
        key('Escape');noise=root/'noise.pdf';noise.write_bytes(noise_pdf())
        remote('open '+str(noise)+';layout_single;goto 1;fit_to_page_width;add_bookmark noise-ready')
        wait(lambda:marker('noise-ready'));time.sleep(.8)
        image=picture();w=window();density=image.width/w['size'][0];page_width=min(w['size'][0]-32,600);center=(w['size'][0]-page_width)/2
        remote('copy_screenshot_to_clipboard;add_bookmark large-image-ready')
        drag(((center+20)*density,102*density),((center+page_width-20)*density,(82+page_width*2/3-20)*density),image)
        wait(lambda:marker('large-image-ready'))
        consumer=subprocess.Popen(['wl-paste','--no-newline','--type','image/png'],stdout=subprocess.PIPE,stderr=subprocess.PIPE)
        time.sleep(.3);remote('add_bookmark during-slow-image-transfer');wait(lambda:marker('during-slow-image-transfer'))
        payload,error=consumer.communicate(timeout=10);assert consumer.returncode==0,error.decode();assert len(payload)>100000
        assert Image.open(io.BytesIO(payload)).width>100
        remote('search research;close;copy;add_bookmark text-after-image')
        wait(lambda:marker('text-after-image'))
        assert subprocess.check_output(['wl-paste','--no-newline','--type','text/plain'],timeout=5).decode().strip()=='research'
        remote('quit');process.wait(timeout=5);assert process.returncode==0
        print('PASS: real rectangle drags/reverse drags on image-only pages, four layouts, cross-page composition/gaps, ordered native PNG export, clipboard PNG equality, no-clobber/source preservation, cancellation, clipboard-command drag, nonblocking large PNG transfer, image-to-text clipboard transition, clean quit')
    finally:
        if held and address:mouse_state('up')
        if consumer is not None and consumer.poll() is None:consumer.terminate();consumer.wait(timeout=5)
        if process.poll() is None:process.terminate();process.wait(timeout=5)
        if process.returncode:log.seek(0);print(log.read())
        if prior_clip is not None:subprocess.run(['wl-copy','--type',prior_type],input=prior_clip,check=True)
        else:subprocess.run(['wl-copy','--clear'],check=True)
        dispatch('hl.dsp.cursor.move({x='+str(cursor['x'])+',y='+str(cursor['y'])+'})')
        if prior:dispatch('hl.dsp.focus({window='+json.dumps('address:'+prior)+'})')
