#!/usr/bin/env python3
"""Test settings through IPC in an isolated reader; focus it only for captures."""
import argparse,json,pathlib,subprocess,tempfile,time
parser=argparse.ArgumentParser()
parser.add_argument('pdf',type=pathlib.Path)
parser.add_argument('--binary',default='target/debug/omapdf')
parser.add_argument('--screenshot',type=pathlib.Path)
args=parser.parse_args()
def hypr(*arguments): return subprocess.check_output(['hyprctl',*arguments],text=True)
def focus(address): hypr('eval','hl.dispatch(hl.dsp.focus({window='+json.dumps('address:'+address)+'}))')
prior=json.loads(hypr('activewindow','-j')).get('address')
with tempfile.TemporaryDirectory(prefix='omapdf-settings-') as temporary:
    root=pathlib.Path(temporary)
    config=root/'config';config.mkdir()
    preferences=config/'prefs_user.config'
    preferences.write_text('# Retain this comment\nunported_preference keep-me\n')
    library=root/'library.json'
    log=open(root/'gui.log','w+')
    process=subprocess.Popen([args.binary,'--library',str(library),'--config',str(config),str(args.pdf.resolve())],stdout=log,stderr=subprocess.STDOUT)
    def alive():
        if process.poll() is not None:
            log.seek(0);raise AssertionError('reader exited: '+log.read())
    def remote(command):
        subprocess.run([args.binary,'--library',str(library),'--execute-command',command],check=True,timeout=5)
    def wait_for(test):
        deadline=time.monotonic()+8
        while time.monotonic()<deadline:
            alive()
            if test():return
            time.sleep(.05)
        log.seek(0);raise AssertionError('timed out; prefs='+preferences.read_text()+'; log='+log.read())
    def capture(path, command=None):
        previous=json.loads(hypr('activewindow','-j')).get('address')
        try:
            window=next(w for w in json.loads(hypr('clients','-j')) if w.get('pid')==process.pid)
            focus(window['address']);time.sleep(.6)
            if command:remote(command)
            time.sleep(.35)
            window=next(w for w in json.loads(hypr('clients','-j')) if w.get('pid')==process.pid)
            x,y=window['at'];w,h=window['size']
            subprocess.run(['grim','-g',f'{x},{y} {w}x{h}',str(path)],check=True)
        finally:
            if previous:focus(previous)
    try:
        wait_for(lambda:library.exists() and bool(json.loads(library.read_text()).get('documents')))
        if prior:focus(prior)
        for _ in range(3):
            remote('settings');time.sleep(.06)
            remote('close');time.sleep(.04)
            remote('settings');time.sleep(.22)
            remote('close');time.sleep(.22)
        remote('settings;set_preference smooth_scroll 0;set_preference ui_animations 0;set_preference render_sharpness 0.6;set_preference scroll_response 0.12;set_preference wheel_scroll_multiplier 1.5;set_preference vertical_move_amount 1.5;set_preference dark_mode_contrast 0.7;set_preference highlight_color_z 0.2 0.4 0.6')
        wait_for(lambda:'smooth_scroll 0' in preferences.read_text())
        content=preferences.read_text()
        assert 'unported_preference keep-me' in content and '# Retain this comment' in content
        assert 'vertical_move_amount 1.5' in content
        assert 'ui_animations 0' in content
        assert 'render_sharpness 0.6' in content
        assert 'scroll_response 0.12' in content
        assert 'wheel_scroll_multiplier 1.5' in content
        if args.screenshot:capture(args.screenshot.with_stem(args.screenshot.stem+'-settings'),'settings')
        remote('reset_preferences;reload_config;close')
        wait_for(lambda:'smooth_scroll 1' in preferences.read_text())
        assert 'unported_preference keep-me' in preferences.read_text()
        if args.screenshot:capture(args.screenshot)
        remote('quit');process.wait(timeout=5)
        assert process.returncode==0
        print('PASS: settings persistence, preserved user config, reset/reload, interrupted panel animations, themed compact toolbar, clean quit')
    finally:
        if process.poll() is None:process.terminate();process.wait(timeout=5)
