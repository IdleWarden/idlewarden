# idlewarden-capture

Frame capture. Governed by [ADR-0005](../../docs/adr/0005-capture.md).

## The three decisions baked in

1. **The window, not the screen.** Survives the window moving and multi-monitor
   setups, and hands `vision` a stable coordinate space for free.
2. **2-4 fps, not 60.** The single biggest simplification available: idle games
   change slowly, so there is no GPU pipeline, no frame pacing, no dropped-frame
   accounting. Building for 60 fps here would be self-harm.
3. **`Arc<Frame>`, never cloned.** Perception downscales once.

## Backends

* `NullBackend`, synthetic frames, works on any OS, keeps the tests
  honest.
* Windows Graphics Capture, **not yet written**. Replaces `NullBackend` in
  Phase 1.

GDI/BitBlt is not an option: it is slow and returns black frames under modern
presentation paths.

## Known limitation, stated on purpose

Exclusive fullscreen is unsupported. Borderless windowed is required, and the
error says so rather than failing mysteriously.
