# Debian packaging

Templates live under `packaging/deb/debian/`. Copy them to a top-level
`debian/` directory when building a Debian source package.

Notes:
- `debian/rules` builds all three binaries with cargo.
- `debian/*.install` splits the artifacts into daemon/cli/gui packages.
- CLI/GUI suggest the daemon; daemon suggests the CLI.
- GUI uses `Enhances` to show up for desktop installs where supported.
