# ADR-0006: Pure-Rust classical vision; anchoring is the hard part

**Status:** Accepted · **Date:** 2026-08-25

## Decision

Perception is **normalised cross-correlation template matching, digit-template
readouts, and colour probes** over **anchored regions of interest**, implemented
in pure Rust (`image` + `imageproc`).

The original decision named `ocrs`/`rten` for reading numbers. It was reversed
before any of it shipped; see *Numbers are read with digit templates, not OCR*
below.

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

## Numbers are read with digit templates, not OCR

**Amended 2026-09-07** (#32). `Extractor::Ocr` was declared and never
implemented. Implementing it as written meant shipping `ocrs` plus `rten` and
model weights, so the question was distribution before it was code. Priced
against the alternative:

| | `ocrs` + `rten` | digit templates |
|---|---|---|
| Installer | tens of MB of weights against a 2.9 MB bundle | 0 |
| First run offline | needs the model present or downloaded | works |
| Weight licence | has to be compatible with what we redistribute | not applicable |
| Old or locked-down machines | an ML runtime, AVX2, execution policy | none |
| New dependency | two crates and a model format | none, the NCC already existed |
| Per-game authoring | none | about ten glyph crops |
| Reads prose | yes | no |

The genre decides it. Idle-game readouts are numerals in one fixed font on a
flat background, which is precisely the case the existing multi-scale NCC
already solves. Paying an ML runtime's distribution cost to read `747` is the
wrong trade, and the one thing OCR buys that templates do not, reading arbitrary
prose, is not something any rule in this project asks for.

`Extractor::Ocr { roi }` is therefore replaced by `Extractor::Digits`, which
carries the glyph templates, a score floor and a declared `value_type`. The old
variant could not have worked anyway: it returned text and left the caller to
hope, and the issue that split this out said so.

Reversing this is cheap if a real plugin ever needs prose: `Digits` stays, an
OCR extractor is added beside it, and only the plugins that need it pay the
weight. That is why the decision is recorded rather than deferred.

### What makes a wrong number harder than a missing one

A readout that fails to read is harmless; the Governor's confidence floor stops
the agent. A readout that returns a *plausible* wrong number walks straight
past it. Three rules follow from that, and each one is a test:

* **Confidence is the weakest glyph, never the average.** One badly-matched
  digit makes the whole number wrong, and an average hides it behind its
  neighbours.
* **One reading, one baseline and one size.** Each scale is read separately and
  the fullest reading wins. Letting matches from different scales compete
  directly ties at the top and lets an accidental two-pixel match decide the
  line; that is not hypothetical, it read `747` as `7477` before the constraint
  existed.
* **A fractional readout with no decimal-separator glyph is refused at load.**
  Without the separator `1.5` reads as `15`: a wrong number that parses, at full
  confidence. Nothing downstream can tell the two apart, so the load is the only
  place it can be caught.

A number that does not parse reports zero confidence rather than a default,
for the same reason: a confident `0` is a lie the Governor cannot see through.

Not decided here, and deliberately left out until a real plugin needs it: suffix
multipliers (`1.2K`, `3M`). They are common in the genre, and adding them
speculatively means guessing at a notation instead of copying one.

## Consequences

* Plugin packages ship image crops of game UI, copyrighted assets, in small
  fragments. Digit glyphs are the same kind of fragment and carry the same
  constraint. Prefer **perceptual hashes/descriptors** over raw crops where they
  suffice, and support **locally generated** templates from the user's own
  install. This constrains the package format, hence its presence in an ADR.
