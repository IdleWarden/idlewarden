# Changelog

All notable changes to `input` will be documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [0.1.0] - 2026-08-25

### Features

- feat(mods): add the bridge protocol library and a reference bepinex mod
- feat(bridge): connect to a game mod over a named pipe
- feat(plugin-api): add the bridge capability, never granted silently
- feat(desktop): scaffold the tauri v2 and angular shell
- feat(core): reduce ui commands into the session state machine
- feat: hot reload, FerrFlow release config, per-crate docs

### Bug Fixes

- fix(mods): resolve BepInEx from its own nuget feed
- fix: give every crate an explicit version so ferrflow can bump them

### Refactoring

- refactor: drop the cli, the desktop app is the only front end
