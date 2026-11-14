#!/usr/bin/env python3
"""Real multi-page text selection, copying and logical annotation operations on Wayland."""
import argparse,io,json,os,pathlib,subprocess,tempfile,time
from PIL import Image
parser=argparse.ArgumentParser();parser.add_argument('--binary',default='target/release/omapdf');parser.add_argument('--no-tools',action='store_true');args=parser.parse_args()
def hypr(*arguments):return subprocess.check_output(['hyprctl',*arguments],text=True)
def dispatch(expression):return hypr('eval','hl.dispatch('+expression+')')
def fixture():
    streams=['1 .75 .75 rg 0 0 300 200 re f\n',' .75 .75 1 rg 0 0 300 200 re f\n']
    streams=[stream+'0 0 0 rg BT /F1 18 Tf 30 150 Td (Alpha Beta Gamma) Tj ET\n' for stream in streams]
    objects=['<< /Type /Catalog /Pages 2 0 R >>','<< /Type /Pages /Kids [3 0 R 5 0 R] /Count 2 >>',
        '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Resources << /Font << /F1 7 0 R >> >> /Contents 4 0 R >>',f'<< /Length {len(streams[0])} >>\nstream\n{streams[0]}endstream',
        '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Resources << /Font << /F1 7 0 R >> >> /Contents 6 0 R >>',f'<< /Length {len(streams[1])} >>\nstream\n{streams[1]}endstream','<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>']
    data='%PDF-1.4\n';offsets=[]
    for i,obj in enumerate(objects):offsets.append(len(data));data+=f'{i+1} 0 obj\n{obj}\nendobj\n'
    xref=len(data);data+=f'xref\n0 {len(objects)+1}\n0000000000 65535 f \n'
    for offset in offsets:data+=f'{offset:010} 00000 n \n'
    data+=f'trailer\n<< /Size {len(objects)+1} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n';return data.encode()
prior=json.loads(hypr('activewindow','-j')).get('address');cursor=json.loads(hypr('cursorpos','-j'));old_type=None;old_clip=None
try:
    types=subprocess.check_output(['wl-paste','--list-types'],timeout=3,text=True,stderr=subprocess.DEVNULL).splitlines()
    old_type=next((t for t in ['image/png','text/plain;charset=utf-8','text/plain'] if t in types),next(iter(types),None))
    if old_type:old_clip=subprocess.check_output(['wl-paste','--no-newline','--type',old_type],timeout=3)
except (subprocess.SubprocessError,OSError):pass
with tempfile.TemporaryDirectory(prefix='omapdf-selection-') as temporary:
    root=pathlib.Path(temporary);config=root/'config';config.mkdir();library=root/'library.json';log=open(root/'reader.log','w+')
    (config/'prefs_user.config').write_text('ui_animations 0\nmax_page_width 600\npage_padding 16\n')
    pdf=root/'span.pdf';pdf.write_bytes(fixture());original=pdf.read_bytes();base=[args.binary,'--library',str(library),'--config',str(config)]
    environment=dict(os.environ,PATH='/nonexistent',XDG_CACHE_HOME=str(root/'cache')) if args.no_tools else None
    process=subprocess.Popen(base+[str(pdf)],stdout=log,stderr=subprocess.STDOUT,env=environment);address=None;held=False
    def state():return json.loads(library.read_text()) if library.exists() else {}
    def document():return next(iter(state()['documents'].values()))
    def marker(name):return any(b['name']==name for b in state().get('bookmarks',[]))
    def wait(test):
        deadline=time.monotonic()+25
        while time.monotonic()<deadline:
            if process.poll() is not None:log.seek(0);raise AssertionError(log.read())
            if test():return
            time.sleep(.05)
        log.seek(0);raise AssertionError('timeout: '+log.read())
    def remote(command):subprocess.run(base+['--execute-command',command],check=True,timeout=5);time.sleep(.25)
    def key(key,mods=''):dispatch('hl.dsp.send_shortcut({window='+json.dumps(address)+',mods='+json.dumps(mods)+',key='+json.dumps(key)+'})');time.sleep(.08)
    def button(value):dispatch('hl.dsp.send_key_state({window='+json.dumps(address)+',mods="",key="mouse:272",state='+json.dumps(value)+'})');time.sleep(.06)
    def window():return next(w for w in json.loads(hypr('clients','-j')) if w.get('pid')==process.pid)
    def picture():
        w=window();x,y=w['at'];width,height=w['size'];return Image.open(io.BytesIO(subprocess.check_output(['grim','-g',f'{x},{y} {width}x{height}','-']))).convert('RGB')
    def boxes(image):
        masks=[]
        for blue in [False,True]:
            mask=Image.new('1',image.size)
            mask.putdata([(180<=r<=205 and 180<=g<=205 and b>245) if blue else (r>245 and 180<=g<=205 and 180<=b<=205) for r,g,b in image.get_flattened_data()])
            masks.append(mask.getbbox())
        return masks
    def move(point,image):
        w=window();density=image.width/w['size'][0];dispatch('hl.dsp.cursor.move({x='+str(round(w['at'][0]+point[0]/density))+',y='+str(round(w['at'][1]+point[1]/density))+'})');time.sleep(.07)
    def drag(a,b,image):
        global held
        move(a,image);button('down');held=True
        for step in range(1,9):move((a[0]+(b[0]-a[0])*step/8,a[1]+(b[1]-a[1])*step/8),image)
        button('up');held=False;time.sleep(.15)
    def copy(name):
        remote('copy;add_bookmark '+name);wait(lambda:marker(name))
        return subprocess.check_output(['wl-paste','--no-newline','--type','text/plain'],timeout=5).decode()
    try:
        wait(lambda:bool(state().get('documents')) and any(w.get('pid')==process.pid for w in json.loads(hypr('clients','-j'))))
        address='address:'+window()['address'];dispatch('hl.dsp.focus({window='+json.dumps(address)+'})');time.sleep(.7)
        for index,layout in enumerate(['spread','horizontal','vertical']):
            key('Escape');remote('close;layout_'+layout+';goto 1;fit_to_page_width')
            if layout=='horizontal':remote('zoom_out;zoom_out;zoom_out;zoom_out')
            time.sleep(.7);image=picture();first,last=boxes(image);assert first and last,(layout,'page colors missing')
            scale=(first[2]-first[0])/300
            a=(first[0]+100*scale,first[1]+43*scale);b=(last[0]+100*scale,last[1]+43*scale)
            if index%2:a,b=b,a
            drag(a,b,image)
            assert copy('copied-'+layout)=='Beta Gamma\nAlpha Beta',(layout,'cross-page copy lost words or page boundaries')
            # Right-clicking the second fragment must preserve the complete selection.
            move((last[0]+100*scale,last[1]+43*scale),image);key('mouse:273');key('Return')
            time.sleep(.3)
            assert subprocess.check_output(['wl-paste','--no-newline','--type','text/plain'],timeout=5).decode()=='Beta Gamma\nAlpha Beta'
            remote('add_highlight B;add_annot_to_selected_highlight one-cross-page-note;add_bookmark highlighted-'+layout)
            wait(lambda:marker('highlighted-'+layout));highlights=document()['highlights']
            assert len(highlights)==2 and {h['page'] for h in highlights}=={0,1},(layout,highlights)
            assert len({h.get('group') for h in highlights})==1 and highlights[0].get('group')
            assert all(h['note']=='one-cross-page-note' for h in highlights)
            assert [h['text'] for h in sorted(highlights,key=lambda h:h['page'])]==['Beta Gamma','Alpha Beta']
            exported=root/(layout+'.json');remote('export '+str(exported)+';add_bookmark exported-'+layout);wait(lambda:marker('exported-'+layout))
            records=json.loads(exported.read_text())['documents'][0]['highlights'];assert len(records)==1
            assert records[0]['selection_begin_y']<200<records[0]['selection_end_y']
            remote('delete_highlight;add_bookmark deleted-'+layout);wait(lambda:marker('deleted-'+layout));assert not document()['highlights']
        # A normal single-page selection resets the old cross-page endpoint.
        key('Escape');remote('layout_single;goto 1;fit_to_page_width');time.sleep(.7)
        image=picture();first,_=boxes(image);scale=(first[2]-first[0])/300
        drag((first[0]+50*scale,first[1]+43*scale),(first[0]+160*scale,first[1]+43*scale),image)
        assert copy('single-page-copy')=='Alpha Beta Gamma'
        assert pdf.read_bytes()==original
        remote('quit');process.wait(timeout=5);assert process.returncode==0
        print('PASS: forward/backward cross-page text drags, spread/horizontal/vertical selection overlays, ordered UTF-8 clipboard copy with page breaks, context selection retention, grouped highlight/comment/export/deletion, single-page endpoint reset, source preservation, clean quit')
    finally:
        if held and address:button('up')
        if process.poll() is None:process.terminate();process.wait(timeout=5)
        if process.returncode:log.seek(0);print(log.read())
        if old_clip is not None:subprocess.run(['wl-copy','--type',old_type],input=old_clip,check=True)
        else:subprocess.run(['wl-copy','--clear'],check=True)
        dispatch('hl.dsp.cursor.move({x='+str(cursor['x'])+',y='+str(cursor['y'])+'})')
        if prior:dispatch('hl.dsp.focus({window='+json.dumps('address:'+prior)+'})')
