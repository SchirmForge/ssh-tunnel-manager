# Phase 7 validation record

Validated on 2026-09-01 against the Fedora 44 development environment. This
record separates automated/source validation from the manual GUI run that the
mockup handoff requires the user to authorize separately.

## Automated and source validation

The following gates pass:

- `cargo fmt --all -- --check`
- `cargo fmt --manifest-path crates/gui-v2/Cargo.toml -- --check`
- `cargo test --workspace --locked` in Fedora 44: 168 passed, 18 live or
  environment-dependent tests ignored, 0 failed
- `cargo test --manifest-path crates/gui-v2/Cargo.toml --locked` in Fedora 44:
  26 passed, 0 failed
- `cargo clippy --workspace --locked --all-targets --all-features -- -D warnings`
  in Fedora 44
- `cargo clippy --manifest-path crates/gui-v2/Cargo.toml --locked --all-targets --
  -D warnings` in Fedora 44
- `cargo build --workspace --release --locked` in Fedora 44
- `cargo build --manifest-path crates/gui-v2/Cargo.toml --release --locked` in
  Fedora 44
- `cargo deny check` for both Cargo workspaces; advisories, bans, licenses, and
  sources pass, with duplicate-version warnings only
- `git diff --check`

Static review confirms:

- Screens 1a/1b, 1c, 1e, 1f, 1g, 1h, and 1i have corresponding GTK views.
  Screen 1d (tray) remains intentionally deferred.
- Profile actions dispatch toolkit-neutral `AppCommand` values through the
  shared action router; a future tray can reuse the same command surface.
- Search, refresh, navigation, profile actions, profile reordering, dialog
  submission, and cancellation have keyboard-accessible actions. Keyboard
  reordering is bounded to the profile's pinned or unpinned section.
- Icon-only and form controls have accessible labels, errors expose alert
  semantics, and status/progress content exposes status semantics.
- Long dialogs and popovers scroll, action groups wrap, and daemon metrics wrap
  for narrow layouts and larger system font metrics.
- Styling does not set a font family or fixed colors. It uses system fonts and
  GTK/libadwaita semantic theme colors.
- WIP controls remain visibly identified and cannot report false success.
- Daemon prompt, error, name, instruction, and status strings remain display
  copy. Control flow is selected from structured event variants, request types,
  IDs, statuses, capabilities, and booleans. The misleading-text and
  contradictory-ID regression tests pass.
- Help and About copy describe the implemented `cli.toml`, `ui.toml`, Secret
  Service, authentication, daemon, and WIP behavior.
- First-launch setup is a pre-runtime gate. Local file/configuration variants and typed field
  codes select snippet/manual/repair behavior; parser diagnostics and authentication tokens
  are not exposed in setup state or debug output. Unit tests cover discovery, validation,
  cancellation, single runtime handoff, and atomic `0600` persistence.

## Manual validation still gated

No GUI was launched or rendered and no screenshot was captured. The following
checks therefore remain pending a separate user authorization:

- compare every in-scope screen with the supplied mockups in system light and
  dark modes;
- exercise complete keyboard traversal, visible focus order, default actions,
  and Escape/cancel behavior at runtime;
- inspect names, roles, state announcements, and live updates with Orca;
- exercise high contrast, enlarged system fonts, narrow-window resizing, and
  long/localized daemon copy;
- exercise real online/offline transitions, supported authentication methods,
  client-held Secret Service credentials, and failure recovery against a live
  daemon;
- exercise no-config, snippet, empty-host, invalid-config, manual mode, cancel/retry and
  post-save first-launch transitions;
- confirm the release binary runs against the Bazzite 44 host GTK/libadwaita
  runtime.

## Cutover remains pending acceptance

The repository still defaults to `crates/gui-gtk`. The following belong to the
separately reviewable cutover after manual acceptance and are not part of this
validation change:

- choose the production executable name and application ID;
- integrate `gui-v2` into the root workspace and default build targets;
- update CI, Makefile, installation, packaging, and user-facing launch docs;
- retire `crates/gui-gtk` and remove the temporary nested workspace boundary.
