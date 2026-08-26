# Desktop app (Tauri v2 + Angular)

The only front end the project ships. Tauri renders an Angular application in
the platform's own web view (WebView2 on Windows, WebKitGTK on Linux) with a
Rust backend in the same process, so capture, vision, input and the Governor all
run natively and only the interface is web.

## The one rule

**No `tauri::` symbol outside this directory, and no business logic inside it.**

The app is an adapter: it turns UI gestures into `idlewarden_core::Command` and
renders `idlewarden_core::Event`. If a screen needs something the event
vocabulary cannot express, extend the vocabulary in the Core, do not reach past
it.

That rule is enforced by `crates/core/tests/pipeline.rs`, which drives a full
session with no UI at all. `src-tauri/src/session.rs` holds a `Mutex<Session>`
and forwards commands to `Session::apply`; it decides nothing itself.

## Running it

```bash
pnpm install
pnpm tauri dev      # Angular dev server on :1420, inside the Tauri window
pnpm tauri build    # bundle for the host platform
pnpm build          # frontend only
pnpm format         # prettier
```

On Linux you need the Tauri system packages first: `libwebkit2gtk-4.1-dev`,
`libayatana-appindicator3-dev`, `librsvg2-dev`, `libxdo-dev`, `libssl-dev`,
`patchelf`. The CI workflow installs exactly that list.

## Layout

```
apps/desktop/
├── src/app/session/    ← the Session screen: state, controls, refusals
├── src/app/            ← shell, routes, global styles
└── src-tauri/          ← the adapter. Four Rust files, no decisions.
```

Components are generated with separate `.ts` / `.html` / `.css` files;
`angular.json` pins `inlineTemplate` and `inlineStyle` to false so
`ng generate component` keeps doing that.

## Updates

`src-tauri/src/updates.rs` asks the cloud endpoint whether a newer build exists.
The channel is a setting in the app rather than a URL to edit, which is the
point: a user opts into beta from the Updates panel and it survives a restart.

It also generates a **per-installation identifier**, stored next to the
settings, and sends it as `x-idlewarden-install`. Staged rollout needs a stable
value to bucket on, and without it the endpoint withholds any partial rollout by
design. It identifies an install, never a person: a random value made locally
and sent to nothing but our own endpoint.

Two things this deliberately does **not** do. It never downloads or installs
anything: that needs the updater plugin and a signing key that does not exist
yet ([#19](https://github.com/IdleWarden/idlewarden/issues/19),
[#20](https://github.com/IdleWarden/idlewarden/issues/20)). And the endpoint
constant is `https://idlewarden.com/api`, a placeholder nobody has registered,
the same one the site carries.

## Known gap

`src/app/session/session.model.ts` restates by hand the shapes that
`idlewarden_core` serialises. Nothing checks the two stay in step. Generating
them (ts-rs, tauri-specta) means putting derive macros on Core types for the
UI's benefit, which cuts against ADR-0004, so it is deliberately unresolved
rather than quietly solved.

## Screens, in build order

1. **Detect**: installed games found, whether a plugin exists for each.
2. **Session**: start/stop, dry-run toggle, live state, kill-switch status.
   _Scaffolded._ It renders the real `Session` and surfaces the Core's
   refusals; detection lands with the capture backend, so the state stays
   `searching` until then.
3. **Activity**: timeline of intents, Governor verdicts and outcomes.
4. **Logs**: filterable structured log stream.
5. **Region editor**: draw ROIs and anchors on a captured frame. This is what
   turns "no plugin exists" into "you just wrote one" (L0 in the README).
6. **Profiles**: per-game configuration and Governor limits.
