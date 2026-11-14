# omapdf-bin AUR package

This recipe repackages the upstream portable executable for x86-64 and ARM64.
It installs the reader, desktop entry, icons, and license notices under `/usr`.
The executable retains its embedded runtime and third-party notices; it needs
host GPU drivers and a Wayland session. No Rust build or FUSE is required.

## Release prerequisite

The `v0.0.1-alpha` release is currently a GitHub draft. Its executables have
been downloaded through authenticated GitHub access and both SHA-256 digests
verified. The source archive is already public. Publish the existing release
with these exact assets before submitting this recipe to AUR. Do not replace
release assets without updating the checksums.

The Arch version `0.0.1alpha` corresponds to upstream `0.0.1-alpha`.

## Build and check

From this directory:

```sh
makepkg --verifysource
makepkg --cleanbuild
bsdtar -tf omapdf-bin-*.pkg.tar.zst
makepkg --printsrcinfo > .SRCINFO
```

Use `namcap PKGBUILD omapdf-bin-*.pkg.tar.zst` when namcap is installed.
For an independent environment, test with Arch devtools in a clean chroot.
ARM64 needs a native ARM64 test machine; it was checksum-verified here only.
Do not commit the downloaded archives, executables, or built packages to AUR.

## Submit

After the release is public and AUR SSH authentication is configured:

```sh
git clone ssh://aur@aur.archlinux.org/omapdf-bin.git /tmp/omapdf-bin-aur
cp PKGBUILD .SRCINFO /tmp/omapdf-bin-aur/
cd /tmp/omapdf-bin-aur
git add PKGBUILD .SRCINFO
git commit -m 'Initial omapdf-bin package'
git push origin master
```

The AUR repository contains the packaging metadata, not the application source
or binary. No install script changes users' default PDF/EPUB associations.
Pacman's normal desktop/icon cache hooks handle integration.

## Updates

Update `pkgver`, every release tag/asset URL and the source archive directory
in `package()` together. Reset `pkgrel` to 1 for an upstream version change.
Verify both architecture downloads against upstream checksums, replace all
three SHA-256 values, rebuild/test, and regenerate `.SRCINFO` before pushing.
The portable artifact must not be stripped: it contains an appended payload.
