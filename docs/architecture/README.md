# Architecture Documentation

How SSH Tunnel Manager is built. For *what it does and for whom*, see
[../user-stories/](../user-stories/). For what is planned, see [../ROADMAP.md](../ROADMAP.md).

| Document | Audience | Contents |
|---|---|---|
| [FUNCTIONAL_ARCHITECTURE.html](FUNCTIONAL_ARCHITECTURE.html) | Anyone wanting to understand the system | Components and their responsibilities, data flow, configuration storage, API behaviour |
| [TECHNICAL_ARCHITECTURE.html](TECHNICAL_ARCHITECTURE.html) | Contributors | Design decisions and rationale, crate layout, concurrency model, dependency strategy, testing strategy |
| [TECHNICAL_REFERENCE.md](TECHNICAL_REFERENCE.md) | Contributors | Module, struct and API-level reference; error handling; build requirements |
| [SECURITY.md](SECURITY.md) | Operators, auditors | Threat model, credential handling, permissions, remote daemon guidance, supply-chain policy and accepted risks, vulnerability reporting |
| [SYSTEM_REQUIREMENTS.md](SYSTEM_REQUIREMENTS.md) | Operators | Supported platforms, runtime and build dependencies, hardware, network, desktop compatibility |
| [SYSTEMD.md](SYSTEMD.md) | Operators | User and system service setup, group access, keyring limitations for system services |

## A note on the HTML documents

The two architecture documents are HTML because they carry diagrams. They are entirely
self-contained: one embedded stylesheet, inline SVG, no external assets, no scripts. They
follow the reader's light or dark system theme and print cleanly.

**GitHub renders `.html` files as source, not as pages.** Open them from a local checkout:

```bash
xdg-open docs/architecture/FUNCTIONAL_ARCHITECTURE.html
```

## Keeping these current

| When you… | Update |
|---|---|
| Add or remove a component, or change what one is responsible for | FUNCTIONAL_ARCHITECTURE |
| Change a design decision, the task model, or the crate layout | TECHNICAL_ARCHITECTURE |
| Add or change a module, public struct or API endpoint | TECHNICAL_REFERENCE |
| Touch permissions, credential handling or the network posture | SECURITY |
| Change a dependency version floor or add a platform requirement | SYSTEM_REQUIREMENTS |

If a change alters observable behaviour, it also needs a user story update in
[../user-stories/](../user-stories/) and an entry in [../CHANGELOG.md](../CHANGELOG.md).
