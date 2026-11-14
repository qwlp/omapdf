#!/usr/bin/env python3
"""Exercise stable document/configuration live reload, recovery and native annotation UI commands."""
import argparse
import json
import os
import pathlib
import subprocess
import tempfile
import time

parser=argparse.ArgumentParser()
parser.add_argument('--binary',default='target/release/omapdf')
parser.add_argument('--no-tools',action='store_true')
args=parser.parse_args()

def hypr(*arguments):
    return subprocess.check_output(['hyprctl',*arguments],text=True)
def dispatch(expression):
    return hypr('eval','hl.dispatch('+expression+')')

def pdf_bytes(label):
    stream=f'BT /F1 18 Tf 30 650 Td ({label} research line) Tj ET\n'
    objects=[
        '<< /Type /Catalog /Pages 2 0 R >>',
        '<< /Type /Pages /Kids [3 0 R 4 0 R 5 0 R 6 0 R 7 0 R] /Count 5 >>',
        *['<< /Type /Page /Parent 2 0 R /MediaBox [0 0 500 800] /Resources << /Font << /F1 8 0 R >> >> /Contents 9 0 R >>']*5,
        '<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>',
        f'<< /Length {len(stream)} >>\nstream\n{stream}endstream',
    ]
    data='%PDF-1.4\n';offsets=[]
    for index,obj in enumerate(objects):
        offsets.append(len(data));data+=f'{index+1} 0 obj\n{obj}\nendobj\n'
    xref=len(data);data+=f'xref\n0 {len(objects)+1}\n0000000000 65535 f \n'
    for offset in offsets:data+=f'{offset:010} 00000 n \n'
    data+=f'trailer\n<< /Size {len(objects)+1} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n'
    return data.encode()

prior=json.loads(hypr('activewindow','-j')).get('address')
cursor=json.loads(hypr('cursorpos','-j'))
with tempfile.TemporaryDirectory(prefix='omapdf-reload-') as temporary:
    root=pathlib.Path(temporary);config=root/'config';config.mkdir()
    preferences=config/'prefs_user.config'
    preferences.write_text('ui_animations 0\nmax_page_width 700\n')
    pdf=root/'live.pdf';pdf.write_bytes(pdf_bytes('Alpha'))
    library=root/'library.json';log=open(root/'reader.log','w+')
    base=[args.binary,'--library',str(library),'--config',str(config)]
    environment=dict(os.environ,PATH='/nonexistent',XDG_CACHE_HOME=str(root/'cache')) if args.no_tools else None
    process=subprocess.Popen(base+[str(pdf)],stdout=log,stderr=subprocess.STDOUT,env=environment)
    def state():return json.loads(library.read_text()) if library.exists() else {}
    def current_id():return next(iter(state().get('recent',[])),None)
    def document():return state()['documents'][current_id()]
    def wait(test):
        end=time.monotonic()+20
        while time.monotonic()<end:
            if process.poll() is not None:
                log.seek(0);raise AssertionError(log.read())
            if test():return
            time.sleep(.05)
        log.seek(0);raise AssertionError('timeout: '+log.read())
    def remote(command):
        subprocess.run(base+['--execute-command',command],check=True,timeout=5)
        time.sleep(.25)
    def key(value):
        dispatch('hl.dsp.send_shortcut({window='+json.dumps(address)+',mods="",key='+json.dumps(value)+'})')
        time.sleep(.1)
    def bookmark(name):
        remote('add_bookmark '+name)
        wait(lambda:any(b['name']==name for b in state()['bookmarks']))
        return next(b['location'] for b in state()['bookmarks'] if b['name']==name)
    try:
        wait(lambda:current_id() and any(w.get('pid')==process.pid for w in json.loads(hypr('clients','-j'))))
        window=next(w for w in json.loads(hypr('clients','-j')) if w.get('pid')==process.pid)
        address='address:'+window['address'];dispatch('hl.dsp.focus({window='+json.dumps(address)+'})')
        remote('search Alpha;add_highlight b;add_annot_to_highlight live-reload-note;close;goto 3;zoom_in;move_down')
        wait(lambda:bool(document()['highlights']) and document()['page']==2)
        time.sleep(.5);before=bookmark('before-reload');old_id=current_id()
        # Preserve mtime and length while atomically changing the inode/content.
        stat=pdf.stat();replacement=root/'replacement.pdf';replacement.write_bytes(pdf_bytes('Bravo'))
        assert replacement.stat().st_size==stat.st_size
        os.utime(replacement,ns=(stat.st_atime_ns,stat.st_mtime_ns));replacement.replace(pdf)
        wait(lambda:current_id()!=old_id)
        after=bookmark('after-reload')
        assert after['page']==before['page']
        assert abs(after['zoom']-before['zoom'])<.001
        assert max(abs(a-b) for a,b in zip(after['offset'],before['offset']))<.03
        assert len(document()['highlights'])==1
        assert all(b['location']['document']==current_id() for b in state()['bookmarks'])
        # A broken rewrite keeps the last readable document and annotations.
        good_id=current_id();pdf.write_bytes(b'incomplete PDF rewrite')
        time.sleep(3.2)
        assert current_id()==good_id
        bookmark('during-invalid-rewrite')
        repaired=pdf_bytes('Delta');pdf.write_bytes(repaired[:len(repaired)//2]);time.sleep(.15)
        with pdf.open('ab') as output:output.write(repaired[len(repaired)//2:])
        wait(lambda:current_id()!=good_id)
        assert document()['page']==2 and len(document()['highlights'])==1
        # Preferences and bindings take effect without any reload command.
        preferences.write_text('ui_animations 0\nmax_page_width 700\npage_layout vertical\nfont_size 18\n')
        keys=config/'keys_user.config';keys.write_text('next_page z\n')
        time.sleep(2.2);key('z')
        wait(lambda:document()['page']==3)
        preferences.write_text('font_size invalid\n')
        time.sleep(1.6);key('z')
        wait(lambda:document()['page']==4)
        preferences.write_text('ui_animations 0\nmax_page_width 700\npage_layout single\nfont_size 14\n')
        keys.write_text('next_page v\n')
        time.sleep(2.2);remote('goto 1');key('v')
        wait(lambda:document()['page']==1)
        # Export completes before the following command in the sequence.
        annotated=root/'annotated.pdf'
        remote('embed_annotations '+str(annotated)+';add_bookmark export-complete')
        wait(lambda:any(b['name']=='export-complete' for b in state()['bookmarks']))
        assert annotated.exists() and annotated.stat().st_size>0
        previous=current_id()
        subprocess.run(base+[str(annotated)],check=True,timeout=5)
        wait(lambda:current_id()!=previous)
        remote('import_annotations;add_bookmark import-complete')
        wait(lambda:any(b['name']=='import-complete' for b in state()['bookmarks']))
        assert len(document()['highlights'])==1
        assert document()['highlights'][0]['note']=='live-reload-note'
        count=len(state()['bookmarks'])
        remote('import_annotations;set_status_string imported-again')
        time.sleep(.5);assert len(document()['highlights'])==1 and len(state()['bookmarks'])==count
        # Palette description search must work without displacing command-name matches.
        remote('execute_command')
        for char in 'add a bookmark to the current location':key('space' if char==' ' else char)
        key('Return')
        for char in 'description-check':key('minus' if char=='-' else char)
        key('Return')
        wait(lambda:any(b['name']=='description-check' for b in state()['bookmarks']))
        # Edit and clear a highlight's comment through the actual prompt.
        remote('search Delta;close;edit_selected_highlight')
        for char in 'edited-note':key('minus' if char=='-' else char)
        key('Return')
        wait(lambda:document()['highlights'][0].get('note')=='edited-note')
        remote('edit_selected_highlight');key('BackSpace');key('Return')
        wait(lambda:not document()['highlights'][0].get('note'))
        remote('add_annot_to_highlight final-note')
        wait(lambda:document()['highlights'][0].get('note')=='final-note')
        edited=root/'edited.pdf'
        remote('embed_annotations '+str(edited)+';set_status_string edit-exported')
        wait(lambda:edited.exists())
        previous=current_id();subprocess.run(base+[str(edited)],check=True,timeout=5)
        wait(lambda:current_id()!=previous)
        remote('import_annotations;set_status_string edit-imported')
        wait(lambda:bool(document()['highlights']))
        assert len(document()['highlights'])==1 and document()['highlights'][0]['note']=='final-note'
        remote('quit');process.wait(timeout=5);assert process.returncode==0
        print('PASS: atomic/same-mtime document reload, view and annotation retention, invalid/partial write recovery, live preferences/keybindings, invalid config recovery, native annotation export/import and note editing/clearing, palette description search, ordered asynchronous commands, idempotence, clean quit')
    finally:
        if process.poll() is None:process.terminate();process.wait(timeout=5)
        if process.returncode:
            log.seek(0);print(log.read())
        dispatch('hl.dsp.cursor.move({x='+str(cursor['x'])+',y='+str(cursor['y'])+'})')
        if prior:dispatch('hl.dsp.focus({window='+json.dumps('address:'+prior)+'})')
