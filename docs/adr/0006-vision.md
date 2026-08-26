# ADR-0006: Pure-Rust classical vision; anchoring is the hard part

**Status:** Accepted · **Date:** 2026-08-25

## Decision

Perception is **normalised cross-correlation template matching, OCR, and colour
probes** over **anchored regions of interest**, implemented in pure Rust
(`image` + `imageproc`, `ocrs`/`rten` for OCR).

**No OpenCV.** ONNX (`ort`) arrives later as an *optional perception plugin*,
never in the Core.

## Why not OpenCV

The C++ dependency on Windows is a build and distribution liability,
vcpkg/CMake in CI, DLLs in the installer, a whole class of "works on my machine"
- for capability we do not need. Idle-game UIs are flat, static and
high-contrast: template matching solves them.

## Why not start with ML

Object detection needs labelled datasets, per game. That cost is invisible when
you plan and crushing when you execute. Classical vision needs one screenshot
crop.

## The actual hard problem

Not matching, **anchoring**. Resolution, DPI scaling, UI language and game
patches each invalidate raw pixel coordinates, and they do it all at once.

Mitigations, designed in from day one because retrofitting them means rewriting
every plugin:

* All regions are **window-relative** (`0.0..=1.0`), never absolute.
* **Multi-scale matching**, so a resolution change degrades instead of failing.
* **Anchors**: visually stable elements located before extraction, producing an
  offset that re-registers every region.
* A lost anchor is a **structural error**; a merely uncertain rule returns **low
  confidence**. These are different failures and the Governor treats them
  differently.

## Consequences

* Plugin packages ship image crops of game UI, copyrighted assets, in small
  fragments. Prefer **perceptual hashes/descriptors** over raw crops where they
  suffice, and support **locally generated** templates from the user's own
  install. This constrains the package format, hence its presence in an ADR.
