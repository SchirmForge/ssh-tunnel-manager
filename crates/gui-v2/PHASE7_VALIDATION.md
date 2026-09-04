# Phase 7 validation and cutover record

Validated on 2026-09-01, 2026-09-02, and 2026-09-03 against the Fedora 44 development
environment and the Bazzite/Fedora 44 host runtime.

## Completed validation

- Static review covers all in-scope mockup views. The tray remains deliberately
  deferred.
- Profile actions dispatch toolkit-neutral `AppCommand` values through the
  shared action router, preserving the command surface for a future tray.
- Styling uses system fonts and GTK/libadwaita semantic colors. WIP controls
  remain visibly identified and cannot report false success.
- Daemon text is display-only. Control flow uses structured event variants,
  request types, request IDs, statuses, capabilities, and booleans. Regression
  tests cover misleading text and contradictory IDs.
- First-launch setup validates or creates `cli.toml` before starting the runtime,
  redacts tokens from diagnostics, and persists the file atomically with mode
  `0600`.
- The release GUI launched on Bazzite, stayed connected beyond the former SSE
  timeout window, reconciled a live daemon with three connected tunnels, and
  operated alongside the CLI.
- The user accepted that `gui-v2` does not regress profile, credential-store,
  daemon, SSE, or authentication behavior.

## Production cutover

- Package and binary: `ssh-tunnel-gui`
- Application ID: `io.github.schirmforge.SshTunnelManager`
- Source: `crates/gui-v2`
- Root workspace: member and default member
- Integration: CI, Makefile, development sandbox, installer, desktop entry, and
  current documentation use the production identity
- Obsolete GUI: `crates/gui-gtk` remains a non-default workspace member on the
  same GTK 4.22/libadwaita 1.9 binding generation. It is not installed or
  packaged, receives no updates, and has no planned removal date.

## Controlled-environment compilation

On 2026-09-02 in the `fedora44-rpmbuild` Fedora 44 distrobox:

- `cargo check -p ssh-tunnel-gui --offline` passed without warnings.
- `cargo check -p ssh-tunnel-gui-gtk --offline` passed. The obsolete GUI emits
  deprecation warnings for its frozen pre-libadwaita-1.6 dialog APIs; these do
  not prevent compilation and will not trigger maintenance work in the frozen
  crate.
- The shared workspace resolves `gtk4` 0.11.4, `libadwaita` 0.9.2, and
  `glib`/`gio` 0.22.9 for both GUI crates.

Final locked validation in Fedora 44 also passed:

- `cargo fmt --all -- --check`
- Default-workspace tests: 214 passed, 19 live/environment tests ignored, 0 failed
- Default-workspace Clippy with all targets/features and warnings denied
- Release builds for both `ssh-tunnel-gui` and `ssh-tunnel-gui-gtk`
- `cargo deny check bans licenses sources` (existing duplicate-version warnings only)
- Shell syntax, desktop-entry validation, architecture HTML validation, and
  `git diff --check` (`shellcheck` was not installed in the validation environment)

## Deferred validation

The user owns interaction-by-interaction validation. The only unfinished Phase 7 review is:

- complete keyboard-only traversal/focus/default/cancel checks;
- compare the implemented screens with the supplied mockups in the relevant
  system appearance modes.

That work is listed publicly in `docs/KNOWN_ISSUES.md`. It is not a production-cutover
blocker and does not reopen the accepted functional areas.

## v0.6.0 follow-up validation

On 2026-09-03, the active-profile edit and structured reconnect changes passed 53
`gui-core` tests and 26 production-GUI tests. Strict Clippy passed for both maintained
crates, and the obsolete GUI still compiled with its known deprecation warnings. Regression
tests confirm that only typed status codes can trigger the post-save reconnect offer.
