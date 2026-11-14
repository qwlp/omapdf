#!/usr/bin/env python3
"""Exercise masked native PDF unlocking, encrypted reload/rekey and session expiry on Wayland.
Generate inputs first: cargo run --locked --example password_fixture -- /tmp/omapdf-password-fixtures
"""
import argparse
import io
import json
import os
import pathlib
import subprocess
import tempfile
import time
from PIL import Image, ImageChops

parser=argparse.ArgumentParser()
parser.add_argument('--binary',default='target/release/omapdf')
parser.add_argument('--fixtures',type=pathlib.Path,default=pathlib.Path('/tmp/omapdf-password-fixtures'))
parser.add_argument('--no-tools',action='store_true')
args=parser.parse_args()
def hypr(*arguments):return subprocess.check_output(['hyprctl',*arguments],text=True)
def dispatch(expression):return hypr('eval','hl.dispatch('+expression+')')
prior=json.loads(hypr('activewindow','-j')).get('address')
cursor=json.loads(hypr('cursorpos','-j'))
with tempfile.TemporaryDirectory(prefix='omapdf-password-') as temporary:
    root=pathlib.Path(temporary);config=root/'config';config.mkdir()
    (config/'prefs_user.config').write_text('ui_animations 0\nmax_page_width 700\n')
    pdf=root/'protected.pdf';pdf.write_bytes((args.fixtures/'locked.pdf').read_bytes())
    library=root/'library.json';log=open(root/'reader.log','w+');process=None
    base=[args.binary,'--library',str(library),'--config',str(config)]
    environment=dict(os.environ,PATH='/nonexistent',XDG_CACHE_HOME=str(root/'cache')) if args.no_tools else None
    def state():return json.loads(library.read_text()) if library.exists() else {}
    def current():return next(iter(state().get('recent',[])),None)
    def document():return state()['documents'][current()]
    def wait(test):
        deadline=time.monotonic()+25
        while time.monotonic()<deadline:
            if process.poll() is not None:log.seek(0);raise AssertionError(log.read())
            if test():return
            time.sleep(.05)
        log.seek(0);raise AssertionError('timeout: '+log.read())
    def remote(command):
        subprocess.run(base+['--execute-command',command],check=True,timeout=5);time.sleep(.25)
    def key(value,mods=''):
        dispatch('hl.dsp.send_shortcut({window='+json.dumps(address)+',mods='+json.dumps(mods)+',key='+json.dumps(value)+'})');time.sleep(.07)
    def type_password(text):
        for character in text:key('minus' if character=='-' else character)
    def bookmark(name):
        remote('add_bookmark '+name);wait(lambda:any(b['name']==name for b in state().get('bookmarks',[])))
        return next(b['location'] for b in state()['bookmarks'] if b['name']==name)
    def focus():
        global address
        wait(lambda:any(w.get('pid')==process.pid for w in json.loads(hypr('clients','-j'))))
        window=next(w for w in json.loads(hypr('clients','-j')) if w.get('pid')==process.pid)
        address='address:'+window['address'];dispatch('hl.dsp.focus({window='+json.dumps(address)+'})');time.sleep(.8)
    def prompt_picture():
        window=next(w for w in json.loads(hypr('clients','-j')) if w.get('pid')==process.pid)
        x,y=map(round,window['at']);width,height=map(round,window['size'])
        time.sleep(.25)
        return Image.open(io.BytesIO(subprocess.check_output(['grim','-g',f'{x},{y+height-150} {width}x130','-']))).convert('RGB')
    def replace(name):
        replacement=root/'replacement.pdf';replacement.write_bytes((args.fixtures/name).read_bytes());replacement.replace(pdf)
    try:
        process=subprocess.Popen(base+[str(pdf),'--execute-command','goto 2;add_bookmark unlocked-startup'],stdout=log,stderr=subprocess.STDOUT,env=environment)
        focus();assert not current()
        type_password('alpha');first=prompt_picture();key('a','CTRL');type_password('bravo');second=prompt_picture()
        assert ImageChops.difference(first,second).getbbox() is None,'password glyphs were not masked consistently'
        key('Return');time.sleep(.8);assert not current(),'wrong password unlocked the PDF'
        type_password('reader-secret');key('Return')
        wait(lambda:any(b['name']=='unlocked-startup' for b in state().get('bookmarks',[])))
        assert document()['page']==1,'startup commands ran before authentication'
        remote('search research;add_highlight b;close')
        wait(lambda:bool(document()['highlights']))
        exported=root/'encrypted-export.pdf';original=pdf.read_bytes()
        remote('embed_annotations '+str(exported)+';add_bookmark exported')
        wait(lambda:any(b['name']=='exported' for b in state().get('bookmarks',[])))
        assert exported.is_file() and pdf.read_bytes()==original
        before=bookmark('before-reload');old_id=current();replace('revised.pdf')
        wait(lambda:current()!=old_id)
        after=bookmark('after-reload');assert after['page']==before['page'] and len(document()['highlights'])==1
        old_id=current();replace('rekeyed.pdf');time.sleep(2.5)
        remote('add_bookmark after-rekey');assert not any(b['name']=='after-rekey' for b in state()['bookmarks'])
        type_password('replacement-secret');key('Return')
        wait(lambda:any(b['name']=='after-rekey' for b in state()['bookmarks']))
        assert current()!=old_id and len(document()['highlights'])==1
        # Cancelling a rekey prompt must not repeatedly interrupt the same readable view.
        old_id=current();replace('locked.pdf');time.sleep(2.5);key('Escape')
        bookmark('cancelled-unlock');time.sleep(3.2);bookmark('still-cancelled')
        assert current()==old_id
        remote('reload;add_bookmark explicit-retry');time.sleep(.8)
        type_password('reader-secret');key('Return')
        wait(lambda:any(b['name']=='explicit-retry' for b in state()['bookmarks']))
        remote('quit');process.wait(timeout=5);assert process.returncode==0
        # A fresh process must ask again. Owner passwords work as well.
        process=subprocess.Popen(base+[str(pdf),'--execute-command','add_bookmark fresh-process'],stdout=log,stderr=subprocess.STDOUT,env=environment)
        focus();time.sleep(.5)
        assert not any(b['name']=='fresh-process' for b in state()['bookmarks'])
        type_password('owner-secret');key('Return')
        wait(lambda:any(b['name']=='fresh-process' for b in state()['bookmarks']))
        remote('open '+str(exported)+';goto 2;add_bookmark protected-open')
        time.sleep(.8)
        assert not any(b['name']=='protected-open' for b in state()['bookmarks'])
        type_password('reader-secret');key('Return')
        wait(lambda:any(b['name']=='protected-open' for b in state()['bookmarks']))
        opened=next(b['location'] for b in state()['bookmarks'] if b['name']=='protected-open')
        assert opened['page']==1 and pathlib.Path(state()['documents'][opened['document']]['path'])==exported
        remote('open_path;add_bookmark cancelled-path-command');time.sleep(.3);key('Escape')
        bookmark('after-path-cancel')
        assert not any(b['name']=='cancelled-path-command' for b in state()['bookmarks'])
        remote('quit');process.wait(timeout=5);assert process.returncode==0
        for path in [library,root/'reader.log',*config.glob('*')]:
            content=path.read_bytes()
            assert all(secret not in content for secret in [b'reader-secret',b'replacement-secret',b'owner-secret',b'bravo']),f'password persisted in {path.name}'
        print('PASS: masked input, wrong-password retry, gated startup commands, lazy text/rendering, encrypted annotation export, same-password live reload, rekey unlock, cancellation, explicit retry, fresh-process/owner unlock, ordered protected opens, path cancellation, no saved credentials, clean quit')
    finally:
        if process is not None and process.poll() is None:process.terminate();process.wait(timeout=5)
        if process is not None and process.returncode:log.seek(0);print(log.read())
        dispatch('hl.dsp.cursor.move({x='+str(cursor['x'])+',y='+str(cursor['y'])+'})')
        if prior:dispatch('hl.dsp.focus({window='+json.dumps('address:'+prior)+'})')
