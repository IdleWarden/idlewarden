# idlewarden-vision

Perception. Governed by [ADR-0006](../../docs/adr/0006-vision.md).

## The hard problem is anchoring, not matching

Template matching on a flat 2D idle-game UI is easy. What is hard is that
resolution, DPI scaling, UI language and game patches each invalidate raw pixel
coordinates, and they do it all at once.

So, from day one and not as a retrofit:

* Every region is **window-relative** (`0.0..=1.0`), never absolute.
* Matching is **multi-scale**, so a resolution change degrades instead of
  failing.
* **Anchors** are located first and produce an offset that re-registers every
  ROI.

## Errors versus low confidence

These are different failures and the Governor treats them differently:

* A **lost anchor** is a `VisionError`, the layout could not be registered at
  all.
* A **merely uncertain** extraction returns a **low `Confidence`**, not an
  error.

Collapsing the two would either spam errors or silently feed the agent garbage.

## No OpenCV

Pure Rust: `image` + `imageproc`, `ocrs`/`rten` for OCR. The C++ dependency on
Windows is a build and distribution liability for capability we do not need.
ONNX arrives later as an optional perception plugin, never in the Core.
