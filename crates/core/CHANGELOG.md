# Changelog

All notable changes to `core` will be documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [26.9.23] - 2026-09-24

## [26.9.22] - 2026-09-20

### Features

- feat(desktop): the editor writes post-conditions that say a counter moved (#70)

## [26.9.21] - 2026-09-20

### Features

- feat(plugins): Cookie Clicker buys upgrades and the building that pays back soonest (#67)
- feat(agent): a post-condition can say a counter moved (#66)

## [26.9.20] - 2026-09-19

### Features

- feat(desktop): drive a session through a mod the user granted (#52)
- feat(core): let a session observe and act through a mod bridge (#51)

## [26.9.19] - 2026-09-18

### Features

- feat(desktop): author intents in the region editor (#47)

## [26.9.18] - 2026-09-18

### Features

- feat(desktop): add the region editor that writes a plugin from a captured frame (#45)
- feat(core): write a declarative plugin from regions drawn on a frame (#44)
- feat(vision): read numeric readouts with digit templates instead of OCR (#41)
- feat/prove the runner starts (#42)
- feat(desktop): add the Profiles screen and persist per-game Governor limits (#37)

## [26.9.7] - 2026-09-07

### Features

- feat(desktop): add the Detect screen, showing which windows a plugin claims (#34)

### Bug Fixes

- fix(desktop): keep the detector available off Windows

## [26.9.5] - 2026-09-05

### Features

- feat(core): assemble a session from a plugin directory
- feat(core): load a plugin's rules into perception, tree and recipes

## [26.9.4] - 2026-09-04

### Features

- feat(core): carry out intents from declared recipes

## [26.9.2] - 2026-09-02

### Features

- feat(detect): match running windows against plugin manifests

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
