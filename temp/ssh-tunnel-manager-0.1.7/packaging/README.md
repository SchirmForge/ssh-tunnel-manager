# Packaging Layout

This folder holds upstream packaging templates and manifests.

Packages (built as separate binaries):
- ssh-tunnel-daemon: background daemon
- ssh-tunnel-cli: command-line client (ssh-tunnel)
- ssh-tunnel-gui: GTK4 desktop app

Dependency intent:
- CLI/GUI suggest the daemon, but do not require it.
- Daemon suggests the CLI.
- GUI is only weakly suggested on desktops (see deb/rpm templates).

Subdirectories:
- deb: Debian packaging templates (debian/ directory)
- rpm: RPM spec file(s)
- flatpak: Flatpak manifest
- systemd: service unit files used by packages
- appstream: AppStream metadata
- desktop: desktop file(s)
