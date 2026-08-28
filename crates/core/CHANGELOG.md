# Changelog

All notable changes to `core` will be documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [26.8.28] - 2026-08-28

### Features

- feat(core): run a session from a tick, on its own thread
- feat(mods): release the mods like everything else, versioned from the project
- feat(desktop): choose an update channel in the app and identify the install

### Bug Fixes

- fix: bring idlewarden-bridge to the version its siblings already carry
- fix(release): pin internal dependencies and let ferrflow rewrite them
- fix: drop caret constraints on internal deps, calendar versions never match them

## [26.8.26] - 2026-08-26

### Breaking Changes

- feat(release)!: version every package with calver-short

### Bug Fixes

- fix: let internal path deps accept any 0.x so release bumps do not break the build
