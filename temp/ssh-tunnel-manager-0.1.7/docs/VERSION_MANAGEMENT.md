# Version Management Guide

This document explains how to manage versions and changelog for the SSH Tunnel Manager project.

## Quick Reference

### Current Version
Check current version: `grep version Cargo.toml | head -1`

### Bump Version

```bash
# Patch version (0.1.1 -> 0.1.2) - Bug fixes, refactoring
./scripts/bump-version.sh patch

# Minor version (0.1.x -> 0.2.0) - New features
./scripts/bump-version.sh minor

# Major version (0.x.x -> 1.0.0) - Breaking changes, stable release
./scripts/bump-version.sh major
```

## When to Bump Versions

### Patch Version (0.1.X → 0.1.X+1)

Increment patch for:
- ✅ Bug fixes
- ✅ Security patches
- ✅ Performance improvements
- ✅ Code refactoring (no functionality changes)
- ✅ Documentation updates
- ✅ Internal architecture improvements
- ✅ Test additions/improvements
- ✅ Dependency updates (minor)

**Examples:**
- Fixed PID file cleanup on daemon exit
- Optimized tunnel connection pooling
- Moved TLS module to common crate (code consolidation)
- Added unit tests for authentication
- Updated dependencies for security patches

### Minor Version (0.X.0 → 0.X+1.0)

Increment minor for:
- ✅ New features or functionality
- ✅ New operational modes
- ✅ New CLI commands or subcommands
- ✅ GUI implementation milestones
- ✅ Significant architecture changes
- ✅ Configuration format changes (breaking)
- ✅ New authentication methods
- ✅ Protocol additions
- ✅ New crates or major modules

**Examples:**
- Added SSH host key verification support
- Implemented GUI with profile management
- Added SOCKS5 dynamic forwarding
- Introduced certificate pinning for HTTPS
- Added remote port forwarding capability
- New daemon operational mode (e.g., systemd integration)

### Major Version (X.0.0 → X+1.0.0)

Increment major for:
- ✅ Complete rewrites
- ✅ Fundamental architecture changes
- ✅ Breaking API changes for library users
- ✅ Production-ready releases (0.x → 1.0)
- ✅ Major feature completions warranting stable release

**Examples:**
- 0.9.x → 1.0.0: First stable release
- 1.x.x → 2.0.0: Complete rewrite with new architecture
- Breaking changes to public API

## Workflow

### 1. Make Changes
Work on your feature, bug fix, or improvement.

### 2. Update CHANGELOG.md
Add your changes under the `[Unreleased]` section:

```markdown
## [Unreleased]

### Added
- New feature description

### Changed
- Modified behavior description

### Fixed
- Bug fix description
```

### 3. Complete Your Work
Ensure:
- ✅ Code compiles
- ✅ Tests pass
- ✅ Documentation is updated

### 4. Decide Version Bump Type
Based on the guidelines above, decide: patch, minor, or major

### 5. Run Version Bump Script

```bash
./scripts/bump-version.sh [patch|minor|major]
```

This will:
- Update version in `Cargo.toml`
- Move `[Unreleased]` changes to new version in `CHANGELOG.md`
- Add date to changelog entry

### 6. Review CHANGELOG.md
Edit the newly created version section in `CHANGELOG.md`:
- Add detailed descriptions
- Organize changes by category
- Ensure clarity for users

### 7. Update PROJECT_STATUS.md (if needed)
For **minor** or **major** versions:
- Update implementation status
- Add/remove features from roadmap
- Update examples and documentation

For **patch** versions:
- Usually no update needed

### 8. Build and Test

```bash
# Build release version
cargo build --release

# Run test suites
./test-network-modes.sh
./test-pidfile.sh

# Manual testing as needed
```

### 9. Commit and Tag

```bash
# Add changed files
git add Cargo.toml CHANGELOG.md PROJECT_STATUS.md

# Commit with descriptive message
git commit -m "Release v0.1.1: TLS consolidation and PID locking"

# Create annotated tag
git tag -a v0.1.1 -m "Release v0.1.1

- Moved TLS module to common crate
- Added PID file locking mechanism
- Fixed multiple daemon instance issues
"

# Push commits and tags
git push
git push --tags
```

## CHANGELOG.md vs PROJECT_STATUS.md

### Use CHANGELOG.md for:
- Version history tracking
- Release notes
- What changed between versions
- User-facing changes
- Migration notes

**Update frequency:** Every version bump

### Use PROJECT_STATUS.md for:
- Current implementation state
- Architecture overview
- What's working vs what's planned
- Developer onboarding
- Big picture status

**Update frequency:** Major milestones, architecture changes

## Changelog Categories

Use these categories in your changelog entries:

### Added
New features, files, capabilities, commands, or functionality

### Changed
Modifications to existing functionality, behavior changes, refactoring

### Deprecated
Features being phased out (still work but will be removed)

### Removed
Deleted features, files, or functionality

### Fixed
Bug fixes, corrections, error handling improvements

### Security
Security-related changes, vulnerability fixes, authentication updates

## Versioning Suggestions

Here are suggested milestones for version numbers:

| Version | Milestone |
|---------|-----------|
| 0.1.0 | Initial implementation, CLI + Daemon working, network modes |
| 0.1.1 | TLS consolidation + PID file locking |
| 0.1.2 | Code consolidation complete (shared common lib) |
| 0.1.3 | SSH host key verification (Task 7) |
| 0.1.x | Additional CLI commands (delete, info, etc.) |
| 0.2.0 | **GUI Phase 1** - Basic window and structure |
| 0.3.0 | GUI feature complete (full CRUD operations) |
| 0.4.0 | Real-time updates via SSE |
| 0.5.0 | System tray integration |
| 0.6.0 | Remote forwarding support |
| 0.7.0 | Dynamic forwarding (SOCKS5) |
| 0.8.0 | Additional features and refinements |
| 0.9.0 | Beta release - production testing |
| 1.0.0 | Production-ready stable release |

These are suggestions - adjust based on actual development priorities.

## Examples

### Example: Patch Release

```bash
# You fixed a bug where daemon crashes on invalid config
./scripts/bump-version.sh patch

# Edit CHANGELOG.md
## [0.1.2] - 2025-12-08

### Fixed
- Daemon no longer crashes when loading invalid configuration
- Improved error messages for config validation

# Commit
git add Cargo.toml CHANGELOG.md
git commit -m "Fix daemon crash on invalid config (v0.1.2)"
git tag -a v0.1.2 -m "Release v0.1.2: Bug fixes"
git push && git push --tags
```

### Example: Minor Release

```bash
# You completed SSH host key verification feature
./scripts/bump-version.sh minor

# Edit CHANGELOG.md
## [0.2.0] - 2025-12-15

### Added
- SSH host key verification with known_hosts support
- New CLI command: ssh-tunnel known-hosts
- Automatic host key fingerprint validation
- Interactive host key acceptance prompts

### Changed
- Updated security model to validate SSH server identity

# Update PROJECT_STATUS.md
- Mark "SSH host key verification" as complete
- Update security features section

# Commit
git add Cargo.toml CHANGELOG.md PROJECT_STATUS.md
git commit -m "Release v0.2.0: SSH host key verification"
git tag -a v0.2.0 -m "Release v0.2.0: SSH host key verification"
git push && git push --tags
```

## Tips

1. **Keep changelog entries concise but clear** - Users should understand what changed
2. **Use links to issues/PRs** if applicable - `Fixed #123: Description`
3. **Group related changes** - Don't list every file change, group by feature
4. **Write from user perspective** - "You can now..." vs "Added function foo()"
5. **Include migration notes** for breaking changes
6. **Date format**: YYYY-MM-DD (ISO 8601)

## Automation

The `bump-version.sh` script automates:
- ✅ Version number updates in Cargo.toml
- ✅ Changelog section creation
- ✅ Date insertion
- ✅ Reminder prompts for additional steps

Manual steps still required:
- Writing meaningful changelog entries
- Updating PROJECT_STATUS.md
- Running tests
- Creating git commits and tags
