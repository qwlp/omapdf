#!/usr/bin/env python3
"""Exercise multiple documents, tab switching/closing and window routing on Wayland."""
import argparse, json, os, pathlib, subprocess, tempfile, time
parser = argparse.ArgumentParser()
parser.add_argument('--binary', default='target/debug/omapdf')
parser.add_argument('--no-tools', action='store_true')
args = parser.parse_args()
binary = str(pathlib.Path(args.binary).resolve())
def hypr(*args):
    return subprocess.check_output(['hyprctl', *args], text=True)
def dispatch(code):
    return hypr('eval', code)
def focus(address):
    dispatch('hl.dispatch(hl.dsp.focus({window='+json.dumps('address:'+address)+'}))')
prior = json.loads(hypr('activewindow', '-j')).get('address')
cursor = json.loads(hypr('cursorpos', '-j'))
with tempfile.TemporaryDirectory(prefix='omapdf-multidoc-') as temporary:
    root = pathlib.Path(temporary)
    config = root/'config'; config.mkdir()
    library = root/'library.json'
    log = open(root/'gui.log', 'w+')
    paths = [str(pathlib.Path('examples/'+name).resolve()) for name in ['reading-lab.pdf', 'large-reading-lab.pdf']]
    base = [binary, '--library', str(library), '--config', str(config)]
    environment = dict(os.environ, PATH='/nonexistent', XDG_CACHE_HOME=str(root/'cache')) if args.no_tools else None
    process = subprocess.Popen(base+paths, stdout=log, stderr=subprocess.STDOUT, env=environment)
    def clients():
        return [w for w in json.loads(hypr('clients', '-j')) if w.get('pid') == process.pid]
    def state():
        return json.loads(library.read_text()) if library.exists() else {}
    def wait(test):
        end = time.monotonic()+20
        while time.monotonic()<end:
            if process.poll() is not None:
                log.seek(0); raise AssertionError(log.read())
            if test(): return
            time.sleep(.1)
        pathlib.Path('/tmp/omapdf-multidoc-timeout.json').write_text(json.dumps(state(),indent=2)); log.seek(0); raise AssertionError('timeout: '+log.read())
    def remote(command):
        subprocess.run(base+['--execute-command', command], check=True, timeout=5)
    def key(name, mods='CTRL'):
        window = clients()[0]
        dispatch('hl.dispatch(hl.dsp.send_shortcut({window='+json.dumps('address:'+window['address'])+',mods='+json.dumps(mods)+',key='+json.dumps(name)+'}))')
        time.sleep(.3)
    try:
        wait(lambda: len(state().get('documents', {})) == 2)
        assert len(clients()) == 1
        focus(clients()[0]['address']); time.sleep(.5)
        remote('goto 3;add_bookmark second-tab')
        wait(lambda: any(b['name']=='second-tab' for b in state().get('bookmarks', [])))
        second = next(b['location']['document'] for b in state()['bookmarks'] if b['name']=='second-tab')
        key('Tab')
        remote('goto 2;add_bookmark first-tab')
        wait(lambda: any(b['name']=='first-tab' for b in state().get('bookmarks', [])))
        first = next(b['location']['document'] for b in state()['bookmarks'] if b['name']=='first-tab')
        assert first != second, 'Ctrl+Tab did not switch documents'
        assert state()['documents'][second]['page'] == 2
        key('w')
        remote('add_bookmark after-close')
        wait(lambda: any(b['name']=='after-close' for b in state().get('bookmarks', [])))
        assert next(b['location']['document'] for b in state()['bookmarks'] if b['name']=='after-close') == second
        # Continuations belong to the destination reader, including repeated CLI commands and nested opens.
        remote('open '+paths[0]+';goto 4;add_bookmark ordered-open')
        wait(lambda:any(b['name']=='ordered-open' for b in state()['bookmarks']))
        target=next(b['location'] for b in state()['bookmarks'] if b['name']=='ordered-open')
        assert target['document']==first and target['page']==3
        assert state()['documents'][second]['page']==2,'open continuation changed the source tab'
        subprocess.run(base+['--execute-command','open_document '+paths[1],'--execute-command','goto 5','--execute-command','add_bookmark repeated-open'],check=True,timeout=5)
        wait(lambda:any(b['name']=='repeated-open' for b in state()['bookmarks']))
        target=next(b['location'] for b in state()['bookmarks'] if b['name']=='repeated-open')
        assert target['document']==second and target['page']==4
        remote('open '+paths[0]+';add_bookmark nested-middle;open '+paths[1]+';goto 6;add_bookmark nested-final')
        wait(lambda:any(b['name']=='nested-final' for b in state()['bookmarks']))
        assert next(b['location']['document'] for b in state()['bookmarks'] if b['name']=='nested-middle')==first
        target=next(b['location'] for b in state()['bookmarks'] if b['name']=='nested-final')
        assert target['document']==second and target['page']==5
        remote('set_preference open_in_new_windows 1')
        wait(lambda: (config/'prefs_user.config').exists() and 'open_in_new_windows 1' in (config/'prefs_user.config').read_text())
        remote('open '+paths[0]+';goto 7;add_bookmark ordered-window')
        wait(lambda: len(clients()) == 2 and any(b['name']=='ordered-window' for b in state()['bookmarks']))
        target=next(b['location'] for b in state()['bookmarks'] if b['name']=='ordered-window')
        assert target['document']==first and target['page']==6
        assert state()['documents'][second]['page']==5,'new-window continuation changed the source tab'
        remote('quit'); process.wait(timeout=10)
        assert process.returncode == 0
        # Check the saved mode for a fresh multi-file launch too.
        process = subprocess.Popen(base+paths, stdout=log, stderr=subprocess.STDOUT, env=environment)
        wait(lambda: len(clients()) == 2)
        remote('quit'); process.wait(timeout=10)
        assert process.returncode == 0
        print('PASS: multi-file startup, tab switch/close, independent positions, bookmarks preserved, ordered open/repeated-command/nested-open/new-window continuations, saved window mode')
    finally:
        if process.poll() is None: process.terminate(); process.wait(timeout=5)
        if prior: focus(prior)
        dispatch('hl.dsp.cursor.move({x='+str(cursor['x'])+',y='+str(cursor['y'])+'})')
