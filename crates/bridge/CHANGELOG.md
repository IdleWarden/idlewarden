# Changelog

All notable changes to `bridge` will be documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [26.9.2] - 2026-09-20

### Features

- feat(bridge): let a mod that cannot serve a pipe connect over a local websocket (#61)

## [26.9.1] - 2026-09-19

### Bug Fixes

- fix(bridge): connect to the pipe namespace the mod actually serves from (#50)

## [26.8.27] - 2026-08-27

### Breaking Changes

- feat(release)!: version every package with calver-short

### Features

- feat(mods): release the mods like everything else, versioned from the project
- feat(desktop): choose an update channel in the app and identify the install

### Bug Fixes

- fix: bring idlewarden-bridge to the version its siblings already carry
- fix(release): pin internal dependencies and let ferrflow rewrite them
- fix: drop caret constraints on internal deps, calendar versions never match them
- fix: let internal path deps accept any 0.x so release bumps do not break the build
