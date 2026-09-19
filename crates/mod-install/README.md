# idlewarden-mod-install

Installs the mod a bridge plugin needs ([ADR-0014](../../docs/adr/0014-bridge.md)).
No mod ships inside the app: the registry names a release URL and a `sha256`, and
this crate turns the downloaded bytes into files in the game's loader folder.

- `Index::release_for` picks the newest release for a plugin and endpoint that is
  not yanked and speaks a protocol the host accepts.
- `Loader::destination` finds where the loader wants it: `BepInEx/plugins/<id>`,
  MelonLoader's shared `Mods`, or `<id>` under Reloaded-II's mods folder. A game
  without the loader is refused, and a `manual` mod is never installed for the user.
- `install` checks the checksum before anything touches the disk, refuses entries
  that would land outside the folder, symlinks, and archives past 64 MiB, then
  unpacks into a staging folder and moves it in place.

Downloading is the caller's job, and so is the explicit click that starts it.
