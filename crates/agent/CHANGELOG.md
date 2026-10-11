# Changelog

All notable changes to `agent` will be documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [26.10.2] - 2026-10-11

### Features

- feat(agent): accept a big literal as an at_least or at_most threshold (#106)

## [26.10.1] - 2026-10-11

### Features

- feat(plugin-api): add a big value for numbers past f64 (#105)

## [26.9.6] - 2026-09-20

### Features

- feat(agent): a post-condition can say a counter moved (#66)

## [26.9.5] - 2026-09-05

### Features

- feat(core): carry out intents from declared recipes

## [26.9.4] - 2026-09-04

### Features

- feat(agent): tick a declared behaviour tree into intents

## [26.8.26] - 2026-08-26

### Breaking Changes

- feat(release)!: version every package with calver-short

### Bug Fixes

- fix: let internal path deps accept any 0.x so release bumps do not break the build
