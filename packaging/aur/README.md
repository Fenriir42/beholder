# AUR package (`beholder-bin`)

`beholder-bin/` repackages the release `.deb` for Arch Linux. It installs:

- `beholder-desktop`, the app itself;
- a `beholder` launcher that starts the app detached from the terminal;
- the menu entry and icons.

## First publication

1. Create an account on <https://aur.archlinux.org> and add your SSH public key in **My Account**.
2. Clone the empty package repository, copy the two files, and push:

   ```sh
   git clone ssh://aur@aur.archlinux.org/beholder-bin.git ~/aur/beholder-bin
   cp packaging/aur/beholder-bin/PKGBUILD packaging/aur/beholder-bin/.SRCINFO ~/aur/beholder-bin/
   cd ~/aur/beholder-bin
   git add PKGBUILD .SRCINFO
   git commit -m "beholder-bin 0.1.3"
   git push
   ```

Then anyone can install it with `yay -S beholder-bin`, and `yay` picks up new versions.

## New release

The GitHub release must be published (not a draft) before the AUR can download it.

1. In `PKGBUILD`, set `pkgver` to the new version and `pkgrel=1`.
2. Run `updpkgsums` (from `pacman-contrib`) to refresh the checksum, then `makepkg --printsrcinfo > .SRCINFO`.
3. Copy both files to the AUR clone, commit `beholder-bin <version>`, and push.
