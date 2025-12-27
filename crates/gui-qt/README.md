# SSH Tunnel Manager - Qt6 GUI with QML

This is the **Qt6/QML implementation** of SSH Tunnel Manager, built with **qmetaobject-rs**. It demonstrates the multi-framework GUI architecture with ~60-70% code reuse from gui-core.

## Current Status

**Work In Progress** - Qt6 is installed and QML files are created, but qmetaobject-rs 0.2.x proc macros need further investigation.

**Current blockers:**
- qmetaobject-rs 0.2.x attribute macros (#[qproperty], #[qsignal], etc.) not working as expected
- May need to use qmetaobject API differently or consider alternative Qt binding (cxx-qt)
- All business logic code is ready (ProfileViewModel integration complete)

**Technology Stack:**
- **qmetaobject-rs**: Rust bindings for Qt6
- **QML**: Declarative UI (Qt Quick)
- **gui-core**: Shared business logic (~60-70% code reuse)

The implementation demonstrates:
- QML-based UI with Rust backend
- AppBackend bridging QML and gui-core
- ~60-70% code reuse through gui-core
- Clean separation: QML for UI, Rust for logic

## Architecture Overview

### Code Reuse (~60-70%)

The gui-qt implementation will reuse the following from **gui-core** (framework-agnostic):

**100% Shared:**
- Profile loading/saving/deletion
- Profile validation
- Configuration file management
- ProfileViewModel (formatted display data)
- AppCore state management
- HTTP client for daemon communication
- SSE event streaming
- Authentication tracking

**Qt-Specific (~30-40%):**
- QML UI files (declarative UI)
- AppBackend QObject (bridges QML and Rust)
- Qt event loop integration
- QML property bindings and signals

### Module Structure

```
crates/gui-qt/
├── qml/
│   ├── main.qml             # Main application window
│   ├── ProfilesList.qml     # Profile list view (to be added)
│   ├── ProfileDialog.qml    # Profile editor dialog (to be added)
│   └── DaemonConfig.qml     # Daemon configuration (to be added)
├── src/
│   ├── main.rs              # Qt/QML application entry point
│   ├── daemon/              # 100% reusable from gui-gtk
│   │   ├── client.rs        # HTTP client for daemon API
│   │   └── sse.rs           # Server-sent events streaming
│   ├── models/              # QObject models for QML
│   │   └── mod.rs           # Profile models, list models
│   └── ui/                  # Rust backend for QML
│       └── mod.rs           # AppBackend and QML property handlers
└── Cargo.toml
```

## Why qmetaobject-rs?

We chose **qmetaobject-rs** for Qt6 integration:

✅ **Simpler Build**: Pure Rust approach without C++ build complexity
✅ **QML Perfect Fit**: UI is forms/lists - QML excels at this
✅ **Clear Separation**: QML for UI, Rust for business logic
✅ **Qt6 Support**: Works with Qt6 (documentation focuses on Qt5 but supports both)
✅ **Rapid Iteration**: QML changes without recompiling Rust

## Prerequisites

### Qt6 Development Files

**IMPORTANT**: Qt6 development files must be installed before building this crate.

**Debian/Ubuntu:**
```bash
sudo apt install qt6-base-dev qt6-declarative-dev qml6-module-qtquick qml6-module-qtquick-controls qml6-module-qtquick-layouts
```

**Fedora:**
```bash
sudo dnf install qt6-qtbase-devel qt6-qtdeclarative-devel
```

**Arch:**
```bash
sudo pacman -S qt6-base qt6-declarative
```

**Verify Installation:**
```bash
qmake6 --version  # or qmake --version on some systems
```

If `qmake6` is not in PATH, you may need to set `QMAKE` environment variable:
```bash
export QMAKE=/usr/lib/qt6/bin/qmake
```

## Implementation Roadmap

1. ✅ **Basic Structure**: QML main window with Rust backend
2. 🔨 **Profile List**: Business logic complete, qmetaobject proc macros need fixing
   - ✅ ProfilesListModel logic using gui-core (SHARED CODE working!)
   - ✅ ProfileItem conversion from ProfileViewModel
   - ✅ ProfilesList.qml UI designed
   - ✅ Main window with navigation drawer
   - ⚠️ qmetaobject-rs 0.2.x proc macros not compiling (investigating alternatives)
3. ⏸️  **Profile Dialog**: Waiting for step 2 resolution
4. 🚧 **Daemon Config**: Configuration UI matching GTK version
5. 🚧 **Event Loop**: Integrate tokio async runtime with Qt event loop
6. 🚧 **SSE Integration**: Connect daemon events to QML property updates
7. 🚧 **Testing**: Ensure feature parity with GTK GUI

## Example Code Reuse

The skeleton files contain extensive comments showing code reuse. For example, from `profiles_list.rs`:

```rust
/// Populate profiles list using ProfileViewModel from gui-core
fn populate_profiles(_state: Rc<AppState>) {
    // Load profiles using gui-core (SHARED CODE)
    let profiles = ssh_tunnel_gui_core::load_profiles().unwrap_or_default();

    for profile in profiles {
        // Get status from AppCore (SHARED)
        let status = { /* same as GTK */ };

        // Create ProfileViewModel (SHARED)
        let view_model = ProfileViewModel::from_profile(&profile, status);

        // Create Qt list item (Qt-SPECIFIC)
        let item = QListWidgetItem::new(&QString::from(&view_model.name));
        item.set_foreground(&view_model.status_color.to_qt_color());
    }
}
```

**Notice**: Only the last 2 lines (Qt widget creation) are framework-specific. Everything else is shared code from gui-core!

## Building the Skeleton

The skeleton can be built to verify the architecture:

```bash
# Check compilation
cargo check -p ssh-tunnel-gui-qt

# Build release binary (shows skeleton message)
cargo build -p ssh-tunnel-gui-qt --release

# Run skeleton
./target/release/ssh-tunnel-qt
```

## For Users

**To use SSH Tunnel Manager now**, please use one of these instead:

- **GTK GUI** (fully functional): `./target/release/ssh-tunnel-gtk`
- **CLI** (fully functional): `./target/release/ssh-tunnel`

The Qt GUI will be implemented in a future release.

## For Developers

When implementing the actual Qt GUI:

1. **Read gui-gtk implementation first** to understand the patterns
2. **Maximize code reuse** - only write Qt-specific widget code
3. **Use ProfileViewModel** for all display data (don't format manually)
4. **Use gui-core validation** - never duplicate validation logic
5. **Use AppCore** for all state management
6. **Reference skeleton comments** showing what's SHARED vs Qt-SPECIFIC

## License

Apache-2.0 - See LICENSE file for details
