#!/usr/bin/env python3
"""Exercise original SQLite migration, native JSON interchange and failed-save recovery on Wayland."""
import argparse
import hashlib
import json
import os
import pathlib
import sqlite3
import subprocess
import tempfile
import time

parser=argparse.ArgumentParser()
parser.add_argument('--binary',default='target/release/omapdf')
parser.add_argument('--no-tools',action='store_true')
args=parser.parse_args()

def hypr(*arguments):return subprocess.check_output(['hyprctl',*arguments],text=True)
def dispatch(expression):return hypr('eval','hl.dispatch('+expression+')')
def pdf_bytes(text):
    stream=f'BT /F1 18 Tf 30 150 Td ({text}) Tj ET\n'
    objects=['<< /Type /Catalog /Pages 2 0 R >>','<< /Type /Pages /Kids [3 0 R 6 0 R] /Count 2 >>',
        '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>',
        '<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>',f'<< /Length {len(stream)} >>\nstream\n{stream}endstream']
    objects.append(objects[2]);data='%PDF-1.4\n';offsets=[]
    for index,obj in enumerate(objects):offsets.append(len(data));data+=f'{index+1} 0 obj\n{obj}\nendobj\n'
    xref=len(data);data+=f'xref\n0 {len(objects)+1}\n0000000000 65535 f \n'
    for offset in offsets:data+=f'{offset:010} 00000 n \n'
    data+=f'trailer\n<< /Size {len(objects)+1} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n'
    return data.encode()

prior=json.loads(hypr('activewindow','-j')).get('address')
cursor=json.loads(hypr('cursorpos','-j'))
with tempfile.TemporaryDirectory(prefix='omapdf-library-') as temporary:
    root=pathlib.Path(temporary);config=root/'config';config.mkdir();state_dir=root/'state';state_dir.mkdir()
    (config/'prefs_user.config').write_text('ui_animations 0\nmax_page_width 700\n')
    source=root/"résumé's research.pdf";source.write_bytes(pdf_bytes('research'))
    destination=root/'destination.pdf';destination.write_bytes(pdf_bytes('destination'))
    source_id=hashlib.sha256(source.read_bytes()).hexdigest();destination_id=hashlib.sha256(destination.read_bytes()).hexdigest()
    source_hash=hashlib.md5(source.read_bytes()).hexdigest();destination_hash=hashlib.md5(destination.read_bytes()).hexdigest()
    (root/'shared').mkdir();(root/'paths').mkdir()
    shared=root/'shared/shared library.sqlite';local=root/'paths/local paths.sqlite'
    with sqlite3.connect(local) as connection:
        connection.execute('CREATE TABLE document_hash(path TEXT,hash TEXT)')
        connection.executemany('INSERT INTO document_hash VALUES (?,?)',[(str(source),source_hash),(str(destination),destination_hash)])
    with sqlite3.connect(shared) as connection:
        connection.executescript('''CREATE TABLE opened_books(path TEXT,zoom_level REAL,offset_x REAL,offset_y REAL);
            CREATE TABLE bookmarks(document_path TEXT,desc TEXT,offset_y REAL,uuid TEXT);
            CREATE TABLE marks(document_path TEXT,symbol TEXT,offset_y REAL,offset_x REAL,zoom_level REAL,uuid TEXT);
            CREATE TABLE highlights(document_path TEXT,desc TEXT,text_annot TEXT,type TEXT,begin_x REAL,begin_y REAL,end_x REAL,end_y REAL,uuid TEXT,modification_time TEXT);
            CREATE TABLE links(src_document TEXT,dst_document TEXT,src_offset_y REAL,src_offset_x REAL,dst_offset_x REAL,dst_offset_y REAL,dst_zoom_level REAL,uuid TEXT);''')
        connection.executemany('INSERT INTO opened_books VALUES (?,?,?,?)',[(source_hash,1.4,0,255),(destination_hash,1.5,0,45)])
        connection.execute('INSERT INTO bookmarks VALUES (?,?,?,?)',(source_hash,'Original résumé 研究',245,'legacy-bookmark'))
        connection.executemany('INSERT INTO marks VALUES (?,?,?,?,?,?)',[(source_hash,'a',250,0,1.4,'legacy-local'),(destination_hash,'G',45,0,1.5,'legacy-global')])
        connection.execute('INSERT INTO highlights VALUES (?,?,?,?,?,?,?,?,?,?)',(source_hash,'research','Original comment 研究','B',-117,45,-52,245,'legacy-highlight','2026-01-01 12:00:00'))
        # Far portal first: page-only proximity would choose the wrong destination.
        connection.executemany('INSERT INTO links VALUES (?,?,?,?,?,?,?,?)',[(source_hash,destination_hash,380,0,0,45,1.5,'far-portal'),(source_hash,destination_hash,255,0,0,245,1.5,'near-portal')])
    database_bytes={path:path.read_bytes() for path in [shared,local]}
    library=state_dir/'library.json';log=open(root/'reader.log','w+')
    process=None
    def base():return [args.binary,'--library',str(library),'--config',str(config)]
    def state():return json.loads(library.read_text()) if library.exists() else {}
    def wait(test):
        end=time.monotonic()+25
        while time.monotonic()<end:
            if process.poll() is not None:log.seek(0);raise AssertionError(log.read())
            if test():return
            time.sleep(.05)
        log.seek(0);raise AssertionError('timeout: '+log.read())
    def remote(command):
        subprocess.run(base()+['--execute-command',command],check=True,timeout=5)
        time.sleep(.25)
    def key(value,mods=''):
        dispatch('hl.dsp.send_shortcut({window='+json.dumps(address)+',mods='+json.dumps(mods)+',key='+json.dumps(value)+'})')
        time.sleep(.1)
    def bookmark(name):
        remote('add_bookmark '+name);wait(lambda:any(b['name']==name for b in state()['bookmarks']))
        return next(b for b in state()['bookmarks'] if b['name']==name)
    def launch(arguments):
        environment=dict(os.environ,PATH='/nonexistent',XDG_CACHE_HOME=str(root/'cache')) if args.no_tools else None
        return subprocess.Popen(base()+arguments,stdout=log,stderr=subprocess.STDOUT,env=environment)
    try:
        process=launch([str(source)])
        wait(lambda:bool(state().get('documents')) and any(w.get('pid')==process.pid for w in json.loads(hypr('clients','-j'))))
        window=next(w for w in json.loads(hypr('clients','-j')) if w.get('pid')==process.pid);address='address:'+window['address']
        dispatch('hl.dsp.focus({window='+json.dumps(address)+'})')
        bookmark('pre-migration')
        command='import_sqlite '+json.dumps([str(shared),str(local)])
        remote(command+';add_bookmark migrated')
        wait(lambda:any(b['name']=='migrated' for b in state()['bookmarks']))
        imported=state()['documents'][source_id]
        assert len(imported['highlights'])==2 and all(h.get('group')=='legacy-highlight' for h in imported['highlights'])
        assert imported['highlights'][0]['note']=='Original comment 研究' and imported['highlights'][0]['kind']=='B'
        assert state()['global_marks']['G']['document']==destination_id
        assert len(imported['portals'])==2 and all(portal.get('source_offset') for portal in imported['portals'])
        assert bookmark('position-check')['location']['page']==1
        assert any(any(b['name']=='pre-migration' for b in json.loads(path.read_text()).get('bookmarks',[]))
            for path in state_dir.glob('omapdf-library-backup-*.json'))
        remote('goto 1;search research;close;add_annot_to_highlight user-edited-note')
        wait(lambda:all(h['note']=='user-edited-note' for h in state()['documents'][source_id]['highlights']))
        count=len(state()['bookmarks'])
        remote(command+';add_bookmark imported-again')
        wait(lambda:any(b['name']=='imported-again' for b in state()['bookmarks']))
        assert len(state()['bookmarks'])==count+1 and len(state()['documents'][source_id]['highlights'])==2
        assert state()['documents'][source_id]['highlights'][0]['note']=='user-edited-note','old database comment overwrote a newer local edit'
        remote('goto_mark');key('g','SHIFT')
        assert bookmark('global-jump')['location']['document']==destination_id
        remote('prev_state;goto_portal;add_bookmark portal-jump')
        wait(lambda:any(b['name']=='portal-jump' for b in state()['bookmarks']))
        portal=next(b for b in state()['bookmarks'] if b['name']=='portal-jump')
        assert portal['location']['document']==destination_id and portal['location']['page']==1,'portal proximity ignored its source coordinates'
        # Failed disk writes retain pending edits for the next successful save.
        state_dir.chmod(0o555)
        remote('add_bookmark save-failed')
        assert not any(b['name']=='save-failed' for b in state()['bookmarks'])
        state_dir.chmod(0o755)
        bookmark('save-recovered')
        assert any(b['name']=='save-failed' for b in state()['bookmarks']),'failed save discarded a pending annotation'
        exported=root/'library export.json'
        remote('export '+str(exported)+';add_bookmark export-complete')
        wait(lambda:any(b['name']=='export-complete' for b in state()['bookmarks']))
        snapshot=json.loads(exported.read_text())
        assert snapshot['omapdf']['version']==1 and len(snapshot['documents'])==2
        source_export=next(d for d in snapshot['documents'] if d['checksum']==source_hash)
        assert len(source_export['highlights'])==1 and source_export['highlights'][0]['uuid']=='legacy-highlight'
        assert not any(b['name']=='export-complete' for b in snapshot['omapdf']['library']['bookmarks'])
        assert all(path.read_bytes()==before for path,before in database_bytes.items())
        remote('import_report');time.sleep(.3);remote('close;quit');process.wait(timeout=5);assert process.returncode==0
        # Restore into an empty library through the running application's command path.
        library=state_dir/'restored.json'
        process=launch([str(source),'--execute-command','import '+str(exported)+';set_status_string restored'])
        wait(lambda:len(state().get('documents',{}))==2 and bool(state().get('source_imports')))
        restored=state();assert restored['documents'][source_id]['highlights'][0]['note']=='user-edited-note'
        assert restored['global_marks']['G']['document']==destination_id
        assert len(restored['bookmarks'])==len(snapshot['omapdf']['library']['bookmarks'])
        remote('goto 2;search research;close;delete_highlight;add_bookmark deletion-complete')
        wait(lambda:any(b['name']=='deletion-complete' for b in state()['bookmarks']))
        assert not state()['documents'][source_id]['highlights']
        assert 'legacy-highlight' in state()['deleted_annotations']
        remote('retry_imports;add_bookmark retry-complete')
        wait(lambda:any(b['name']=='retry-complete' for b in state()['bookmarks']))
        assert not state()['documents'][source_id]['highlights'], 'retry resurrected a deleted cross-page highlight'
        remote('quit');process.wait(timeout=5);assert process.returncode==0
        print('PASS: read-only custom split SQLite migration, Unicode/UUID preservation, local/global marks and portals, exact source proximity, cross-page grouping/edit/delete/retry, local edit retention, failed-save recovery, ordered JSON export, native restore, source byte preservation, clean quit')
    finally:
        state_dir.chmod(0o755)
        if process is not None and process.poll() is None:process.terminate();process.wait(timeout=5)
        if process is not None and process.returncode:log.seek(0);print(log.read())
        dispatch('hl.dsp.cursor.move({x='+str(cursor['x'])+',y='+str(cursor['y'])+'})')
        if prior:dispatch('hl.dsp.focus({window='+json.dumps('address:'+prior)+'})')
