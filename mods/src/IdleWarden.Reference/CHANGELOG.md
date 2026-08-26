# Changelog

All notable changes to `mod-reference` will be documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [26.8.26] - 2026-08-26

### Breaking Changes

- feat(release)!: version every package with calver-short

### Features

- feat(mods): release the mods like everything else, versioned from the project
- feat(desktop): choose an update channel in the app and identify the install
- feat(mods): add the bridge protocol library and a reference bepinex mod
- feat(bridge): connect to a game mod over a named pipe
- feat(plugin-api): add the bridge capability, never granted silently
- feat(desktop): scaffold the tauri v2 and angular shell
- feat(core): reduce ui commands into the session state machine
- feat: hot reload, FerrFlow release config, per-crate docs

### Bug Fixes

- fix: drop caret constraints on internal deps, calendar versions never match them
- fix: let internal path deps accept any 0.x so release bumps do not break the build
- fix(mods): resolve BepInEx from its own nuget feed
- fix: give every crate an explicit version so ferrflow can bump them

### Refactoring

- refactor: drop the cli, the desktop app is the only front end
