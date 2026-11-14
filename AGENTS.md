# Instructions for future agents

## Keep the installed application current

The user explicitly wants agents to update their installed Omapdf after
finishing application changes. Updating only the repository or `target/release`
does not complete an implementation task.

After changing application code, runtime assets, dependencies, or packaging:

1. Finish the implementation and run appropriate checks. Normally use:
   ```sh
   cargo fmt --all --check
   cargo clippy --locked --all-targets -- -D warnings
   cargo test --locked
   ```
   Run relevant Wayland smoke scripts in `scripts/` for interaction changes.
   Use isolated settings/library files and restore desktop focus and cursor.

2. Build the release reader, then regenerate the portable executable:
   ```sh
   cargo build --release --locked
   python3 scripts/build_portable.py
   ```
   The portable builder embeds the existing release binary. Running it before
   the release build finishes can package stale code.

3. Install **`dist/omapdf`**, the self-contained portable executable, at:
   ```text
   $HOME/.local/bin/omapdf
   ```
   Back up the existing installed binary before replacing it. Stage the new
   executable beside the destination, set mode `0755`, then atomically rename
   it into place. Do not overwrite a running executable in place. Do not
   install `target/debug/omapdf` or substitute the unpackaged release
   binary. The `make install` target builds and installs the portable artifact with backups
   and atomic replacement.

4. Verify that the installed binary matches the artifact and runs:
   ```sh
   cmp dist/omapdf $HOME/.local/bin/omapdf
   $HOME/.local/bin/omapdf --version
   ```

5. Report the implementation, verification, and installed-copy update in the
   final response. If installation is blocked, identify the blocker and say
   explicitly that the installed copy has not been updated.

Documentation-only changes do not require rebuilding or reinstalling unless
the user requests it. Do not kill or restart the user's running reader without
authorization; mention when reopening it is needed to use the new build.

## Desktop integration

The installed desktop entry is:
```text
$HOME/.local/share/applications/io.omapdf.desktop
```

Its command must point to the installed executable. Resolve `$HOME` to the
absolute home directory before writing the desktop file (desktop entries do not
expand environment variables):
```ini
Exec=$HOME/.local/bin/omapdf %f
TryExec=$HOME/.local/bin/omapdf
```

Keep this space-free executable path unquoted: this machine's `xdg-mime`
implementation incorrectly skips quoted executable names, even though the
desktop entry validates and GIO accepts them.

The user selected `io.omapdf.desktop` as their default PDF handler.
Preserve the desktop entry ID and do not alter unrelated MIME associations.
When changing desktop integration, validate it with:
```sh
desktop-file-validate $HOME/.local/share/applications/io.omapdf.desktop
update-desktop-database $HOME/.local/share/applications
xdg-mime query default application/pdf
gio mime application/pdf
```
Respect any newer user instructions about their preferred application.

## Permissions and portability

The user's request already authorizes updating this user-local installation
after implementation work; do not ask for the same permission again. If the
tool sandbox requires escalation for writes under `~/.local/` or runtime cache
creation, request tool-level escalation with a specific explanation. Honor
actual approval requirements and report any unresolved rejection.

Use the Omarchy skill when changing desktop/system configuration. Back up
existing configuration before editing it. Do not modify `/usr/share/omarchy/`.

Keep PDF/EPUB processing native through the bundled MuPDF engine. Do not add
runtime requirements for Poppler commands, Calibre, or repository-local files.
The portable artifact has a static launcher and an embedded runtime; the host
still provides its Wayland compositor and GPU drivers. Preserve required
third-party license notices when changing the package.
