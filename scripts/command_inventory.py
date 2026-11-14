#!/usr/bin/env python3
"""Regenerate Omapdf's command palette and supported-command reference."""
import json
import pathlib
import re
import subprocess

root = pathlib.Path(__file__).resolve().parent.parent
source = (root / 'src/reader.rs').read_text()
dispatcher = source[source.index('        match name {'):source.index('\n    fn key')]
commands = set()
for arm in re.finditer(r'^\s*((?:"[a-z_]+"(?:\s*\|\s*)?)+)(?:\s+if[^=]+)?\s*=>', dispatcher, re.M):
    commands.update(re.findall(r'"([a-z_]+)"', arm.group(1)))
commands = sorted(commands)
descriptions = json.loads((root / 'assets/command_docs.json').read_text())
missing = set(commands) - descriptions.keys()
if missing:
    raise SystemExit(f'Add descriptions to assets/command_docs.json: {sorted(missing)}')
text = '/// Commands supported by Omapdf.\npub const COMMANDS: &[&str] = &[\n'
text += ''.join('    ' + json.dumps(command) + ',\n' for command in commands) + '];\n'
text += '\n/// Human-readable command help.\npub fn description(command: &str) -> &\'static str {\n    match command {\n'
for command in commands:
    text += '        ' + json.dumps(command) + ' => ' + json.dumps(descriptions[command], ensure_ascii=False) + ',\n'
text += '        _ => "",\n    }\n}\n'
(root / 'src/commands.rs').write_text(text)
lines = ['# Omapdf commands', '', 'These commands are available in the palette, keybindings and `--execute-command`.', '', '| Command | Action |', '| --- | --- |']
lines += [f'| `{command}` | {descriptions[command]} |' for command in commands]
(root / 'docs/COMMANDS.md').write_text('\n'.join(lines) + '\n')
subprocess.run(['rustfmt', '--edition', '2024', str(root / 'src/commands.rs')], check=True)
