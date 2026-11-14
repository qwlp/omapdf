#!/usr/bin/env python3
"""Verify continuous line highlights and actual intermediate page-jump/zoom frames."""
import argparse,hashlib,io,json,pathlib,subprocess,tempfile,time
from PIL import Image
parser=argparse.ArgumentParser();parser.add_argument('--binary',default='target/debug/omapdf');args=parser.parse_args()
def hypr(*args):return subprocess.check_output(['hyprctl',*args],text=True)
def dispatch(expr):return hypr('eval','hl.dispatch('+expr+')')
prior=json.loads(hypr('activewindow','-j')).get('address');cursor=json.loads(hypr('cursorpos','-j'))
with tempfile.TemporaryDirectory(prefix='omapdf-motion-') as temporary:
    root=pathlib.Path(temporary);config=root/'config';config.mkdir();library=root/'library.json'
    (config/'prefs_user.config').write_text('max_page_width 600\nui_animations 1\nhighlight_color_a 1 1 0\n')
    texts=['BT /F1 18 Tf 72 650 Td (alpha beta gamma) Tj 0 -26 Td (delta epsilon) Tj ET','BT /F1 24 Tf 72 500 Td (Destination page) Tj ET']
    objects=['<< /Type /Catalog /Pages 2 0 R >>','<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>',
        '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 7 0 R >> >> /Contents 5 0 R >>',
        '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 7 0 R >> >> /Contents 6 0 R >>',
        *[f'<< /Length {len(text)} >>\nstream\n{text}\nendstream' for text in texts], '<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>']
    data='%PDF-1.7\n';offsets=[]
    for i,obj in enumerate(objects):offsets.append(len(data));data+=f'{i+1} 0 obj\n{obj}\nendobj\n'
    xref=len(data);data+=f'xref\n0 {len(objects)+1}\n0000000000 65535 f \n'
    for offset in offsets:data+=f'{offset:010} 00000 n \n'
    data+=f'trailer\n<< /Size {len(objects)+1} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n'
    pdf=root/'motion.pdf';pdf.write_text(data);base=[args.binary,'--library',str(library),'--config',str(config)]
    log=open(root/'log','w+');process=subprocess.Popen(base+[str(pdf)],stdout=log,stderr=subprocess.STDOUT)
    def clients():return [w for w in json.loads(hypr('clients','-j')) if w.get('pid')==process.pid]
    def state():return json.loads(library.read_text()) if library.exists() else {}
    def document():return next(iter(state()['documents'].values()))
    def wait(test):
        end=time.monotonic()+15
        while time.monotonic()<end:
            if process.poll() is not None:log.seek(0);raise AssertionError(log.read())
            if test():return
            time.sleep(.05)
        log.seek(0);raise AssertionError('timeout: '+log.read())
    def remote(command):subprocess.run(base+['--execute-command',command],check=True,timeout=5)
    def capture():
        w=clients()[0];x,y=w['at'];width,height=w['size']
        return Image.open(io.BytesIO(subprocess.check_output(['grim','-g',f'{x},{y} {width}x{height}','-']))).convert('RGB')
    def frames():
        result=[]
        for _ in range(12):result.append(capture());time.sleep(.025)
        return result
    try:
        wait(lambda:clients() and bool(state().get('documents')))
        address='address:'+clients()[0]['address'];dispatch('hl.dsp.focus({window='+json.dumps(address)+'})');time.sleep(.5)
        remote('search alpha beta gamma delta epsilon;add_highlight a;close')
        wait(lambda:bool(document()['highlights']));time.sleep(.5)
        picture=capture();picture.save('/tmp/omapdf-line-highlight.png')
        first,second=document()['highlights'][0]['rects'][:2];scale=600/612
        gap=(first['x']+first['width']+second['x'])/2
        x=int((picture.width-600)/2+gap*scale);y=int(82+(first['y']+first['height']*.45)*scale)
        color=picture.getpixel((x,y))
        assert color[0]>220 and color[1]>220 and 130<color[2]<235, f'word gap is not a soft continuous highlight: {color}'
        remote('goto 2');jump=frames();wait(lambda:document()['page']==1)
        signatures={hashlib.sha256(frame.crop((0,80,frame.width,frame.height-40)).tobytes()).hexdigest() for frame in jump}
        assert len(signatures)>=3, 'page jump had no intermediate fade frames'
        remote('fit_to_page_width');time.sleep(.4)
        baseline=capture()
        def white_width(frame):return max(sum(min(frame.getpixel((x,y)))>245 for x in range(frame.width)) for y in [90,100,110])
        start=white_width(baseline);remote('zoom_in');zoom=frames();finish=white_width(zoom[-1])
        widths=[white_width(frame) for frame in zoom]
        assert finish>start+50 and any(start+5<w<finish-5 for w in widths), f'zoom snapped instead of interpolating: {start=} {widths=}'
        remote('quit');process.wait(timeout=5);assert process.returncode==0
        print('PASS: soft line highlight fills word gaps, page jumps have intermediate fade frames, zoom has intermediate widths, clean quit')
    finally:
        if process.poll() is None:process.terminate();process.wait(timeout=5)
        dispatch('hl.dsp.cursor.move({x='+str(cursor['x'])+',y='+str(cursor['y'])+'})')
        if prior:dispatch('hl.dsp.focus({window='+json.dumps('address:'+prior)+'})')
