# ADR-0005 — Windows Graphics Capture, window-scoped, 2–4 fps

**Status:** Accepted · **Date:** 2026-08-25

## Decision

* Capture via **Windows Graphics Capture**, not BitBlt/GDI.
* Capture the **game window**, never the whole screen.
* Target **2–4 fps**.
* Frames are `Arc<Frame>`, BGRA, never cloned. Perception downscales once.
* **Exclusive fullscreen is unsupported**; borderless windowed is required and
  the error message says so.

## Why

* GDI capture is slow and returns black frames under many modern presentation
  paths. WGC is the supported API and handles composition correctly.
* Window-scoped capture survives the window being moved and multi-monitor
  setups, and it hands the vision layer a stable coordinate space for free.
* **The frame rate is the single biggest simplification available.** Idle games
  change slowly; at 2–4 fps there is no need for a zero-copy GPU pipeline, frame
  pacing, or dropped-frame accounting. Building a 60 fps pipeline for a genre
  that idles would be pure self-harm.

## Consequences

* Anything needing reaction times under ~250 ms is out of scope, by design.
* Capture runs on a dedicated blocking thread and publishes into a `watch`
  channel: the consumer wants the *latest* frame, never a backlog.
