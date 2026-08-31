# Security policy

SSH Tunnel Manager handles SSH credentials and opens network listeners, so security
reports are taken seriously and looked at promptly.

## Reporting a vulnerability

**Please do not open a public issue for a security problem.**

Use GitHub's [private vulnerability reporting](https://github.com/SchirmForge/ssh-tunnel/security/advisories/new)
on this repository. That creates a private advisory only the maintainers can see.

Useful things to include, as far as you can:

- what an attacker can do, and what they need in order to do it
- the version or commit you tested
- steps to reproduce, or a proof of concept
- whether you have told anyone else

You will get an acknowledgement within a few days. If a fix is warranted, the advisory
will say what changed and credit you unless you would rather stay anonymous.

## What is in scope

The daemon, the CLI, the GTK front-end, and the shared crates underneath them. In
particular:

- the SSH client path: host key verification, authentication, port forwarding
- the daemon's HTTP/HTTPS API, its token handling and its TLS certificate pinning
- credential storage in the system keychain
- file and socket permissions

## Known accepted risks

Not everything reported by a scanner has a fix. Where the project knowingly carries a
risk, it is recorded in [`deny.toml`](deny.toml) with a written reason and the
condition under which it should be revisited — never as a bare suppression.

The current list is short, and `cargo deny check` fails the build on anything not on it:

| Advisory | Why it is accepted |
|---|---|
| `RUSTSEC-2023-0071` (`rsa`, Marvin timing attack) | No fixed version exists in **any** `rsa` release, including the 0.10 line. The only escape is dropping RSA key support, which would break users who hold RSA keys. Exposure is narrower than the title suggests: the attack targets RSA *decryption*, while SSH public-key authentication uses *signing*. |
| `RUSTSEC-2025-0134` (`rustls-pemfile`, unmaintained) | Unmaintained but functional, with no known vulnerability. Its functionality moved into `rustls-pki-types`; migrating is a scheduled cleanup, not a fix. |

## How dependencies are kept current

- `Cargo.lock` is committed, so builds are reproducible and what shipped can be audited.
- Dependabot opens weekly lockfile-only pull requests for `cargo`, and keeps the pinned
  GitHub Actions current.
- `cargo deny check` runs on every pull request and **fails the build** on a new
  advisory, a disallowed licence, or a dependency from an unapproved source.
- Git dependencies are banned outright. One has no semver contract, and `cargo audit`
  matches crates.io name and version — so it cannot see a git dependency at all. A clean
  advisory report only means something if every dependency is one the scanner can read.
- GitHub Actions are pinned to commit SHAs rather than tags, and the Rust toolchain is
  pinned by `rust-toolchain.toml`. These workflows handle repository secrets.
- `gitleaks` scans every pull request, so no credential reaches the repository.

Run the same checks locally with `make audit`.
