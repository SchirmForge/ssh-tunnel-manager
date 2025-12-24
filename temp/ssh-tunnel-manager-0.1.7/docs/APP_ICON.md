# Application Icon Design

## Current Status

The SSH Tunnel Manager GUI currently uses the generic GTK4 application icon. We need a custom application icon that:

1. **Represents SSH/tunneling** - Conveys the purpose of the application
2. **Follows GNOME HIG** - Adheres to GNOME Human Interface Guidelines for app icons
3. **Works across DEs** - Looks good on both GNOME and KDE
4. **Scalable** - SVG format for crisp rendering at all sizes

## Icon Concept Ideas

### Option 1: Tunnel + Network
- Visual metaphor: A tunnel with network lines going through it
- Colors: Blue/teal (networking) + gray/dark (tunnel)
- Style: Rounded, modern libadwaita style
- Symbolism: Clearly represents SSH tunneling

### Option 2: Lock + Arrows
- Visual metaphor: A padlock with bidirectional arrows
- Colors: Green (secure) + blue (networking)
- Style: Simple, symbolic
- Symbolism: Security + data flow

### Option 3: Terminal + Shield
- Visual metaphor: Terminal window with shield overlay
- Colors: Terminal green/black + shield blue
- Style: Developer-focused
- Symbolism: SSH (terminal) + security (shield)

### Option 4: Key + Tunnel (Recommended)
- Visual metaphor: SSH key icon with tunnel/pipe element
- Colors: GNOME blue palette + gradient
- Style: Modern, rounded corners, subtle gradient
- Symbolism: SSH key authentication + tunneling

## Implementation Requirements

### File Locations

Following GNOME/FreeDesktop standards:

```
data/icons/
├── hicolor/
│   ├── scalable/
│   │   └── apps/
│   │       └── com.github.ssh-tunnel-manager.svg
│   ├── symbolic/
│   │   └── apps/
│   │       └── com.github.ssh-tunnel-manager-symbolic.svg
│   ├── 16x16/
│   │   └── apps/
│   │       └── com.github.ssh-tunnel-manager.png
│   ├── 32x32/
│   │   └── apps/
│   │       └── com.github.ssh-tunnel-manager.png
│   ├── 48x48/
│   │   └── apps/
│   │       └── com.github.ssh-tunnel-manager.png
│   ├── 64x64/
│   │   └── apps/
│   │       └── com.github.ssh-tunnel-manager.png
│   ├── 128x128/
│   │   └── apps/
│   │       └── com.github.ssh-tunnel-manager.png
│   └── 256x256/
│       └── apps/
│           └── com.github.ssh-tunnel-manager.png
```

### Icon Types Needed

1. **Application Icon (Color)** - `com.github.ssh-tunnel-manager.svg`
   - Full color version for app launchers
   - Follows GNOME color palette
   - Size: Scalable SVG + PNG exports (16px to 256px)

2. **Symbolic Icon (Monochrome)** - `com.github.ssh-tunnel-manager-symbolic.svg`
   - Single color, adapts to theme
   - Used in header bars, menus, system tray
   - Simple, recognizable at small sizes

## Design Guidelines

### GNOME HIG Requirements

1. **Size:** 128×128px canvas with 16px margin
2. **Perspective:** Flat or slightly angled (not full 3D)
3. **Colors:** Use GNOME blue palette:
   - Primary: `#3584e4` (GNOME blue)
   - Dark: `#1c71d8`
   - Light: `#62a0ea`
4. **Shadows:** Subtle drop shadow for depth
5. **Rounded corners:** 4-8px radius
6. **Grid alignment:** Elements on 8px grid

### Symbolic Icon Requirements

1. **Simple shapes:** 1-2 main elements maximum
2. **16px readable:** Must be clear at 16×16px
3. **Single color:** Pure black (#000000)
4. **Stroke width:** 2px minimum at 16px size
5. **Padding:** 2px from canvas edge

## Tools for Creation

### Recommended: Inkscape
- Free and open source
- Native SVG support
- Good for icon design
- GNOME icon templates available

### Alternative: GIMP
- For raster versions
- Export PNG at various sizes
- Can trace SVG for symbolic version

### Icon Template Resources

- **GNOME HIG Icon Templates:** https://gitlab.gnome.org/GNOME/gnome-icon-theme-extras
- **Libadwaita Demo Icons:** Check libadwaita demo app for reference
- **Icon Preview:** Use `Icon Library` app (GNOME) to test

## Integration Steps

### 1. Create Icons

```bash
# Directory structure
mkdir -p data/icons/hicolor/{scalable,symbolic,16x16,32x32,48x48,64x64,128x128,256x256}/apps/

# Create SVG in Inkscape (main color icon)
inkscape data/icons/hicolor/scalable/apps/com.github.ssh-tunnel-manager.svg

# Create symbolic variant
inkscape data/icons/hicolor/symbolic/apps/com.github.ssh-tunnel-manager-symbolic.svg

# Export PNG sizes from SVG
for size in 16 32 48 64 128 256; do
  inkscape --export-width=$size \
           --export-filename=data/icons/hicolor/${size}x${size}/apps/com.github.ssh-tunnel-manager.png \
           data/icons/hicolor/scalable/apps/com.github.ssh-tunnel-manager.svg
done
```

### 2. Install Icons (Linux)

```bash
# System-wide install
sudo install -Dm644 data/icons/hicolor/scalable/apps/com.github.ssh-tunnel-manager.svg \
  /usr/share/icons/hicolor/scalable/apps/

# Update icon cache
sudo gtk-update-icon-cache -f -t /usr/share/icons/hicolor/
```

### 3. Set Window Icon (GTK Code)

```rust
// In crates/gui/src/ui/window.rs
pub fn build(app: &adw::Application) -> adw::ApplicationWindow {
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("SSH Tunnel Manager")
        .icon_name("com.github.ssh-tunnel-manager")  // Add this line
        .default_width(1000)
        .default_height(700)
        .build();

    // ... rest of code
}
```

### 4. Desktop Entry

```desktop
[Desktop Entry]
Name=SSH Tunnel Manager
Comment=Manage SSH tunnels with a modern GUI
Icon=com.github.ssh-tunnel-manager
Exec=ssh-tunnel-gui
Terminal=false
Type=Application
Categories=Network;Utility;
Keywords=ssh;tunnel;port-forwarding;
```

## Temporary Solution (Current)

For Phase 1 development, we're using stock icons:
- `list-add-symbolic` for empty profile list
- `document-properties-symbolic` for "select a profile" placeholder

These will be replaced with custom icons in a future phase.

## Next Steps

1. **Phase 1 (Current):** Use stock symbolic icons ✅
2. **Phase 2:** Design and create application icon (SVG)
3. **Phase 3:** Create symbolic variant
4. **Phase 4:** Export PNG sizes and integrate
5. **Phase 5:** Submit for inclusion in app stores (Flathub, etc.)

## References

- [GNOME HIG: App Icons](https://developer.gnome.org/hig/guidelines/app-icons.html)
- [FreeDesktop Icon Theme Specification](https://specifications.freedesktop.org/icon-theme-spec/icon-theme-spec-latest.html)
- [Libadwaita Icon Guidelines](https://gnome.pages.gitlab.gnome.org/libadwaita/doc/main/styles-and-appearance.html#app-icons)
- [Inkscape Icon Templates](https://gitlab.gnome.org/Teams/Design/icon-development-kit)

## Icon Candidate Sketches

```
Option 4 (Recommended) - Key + Tunnel:

   ╔═══════════════════╗
   ║   [Application]   ║
   ║                   ║
   ║     ┌─────┐       ║
   ║    ┌┤ Key ├┐      ║  ← SSH Key shape
   ║    │└─────┘│      ║
   ║    │   ═══════════╗  ← Tunnel/pipe
   ║    └───────═══════╣
   ║            ═══════╝
   ║                   ║
   ╚═══════════════════╝

   Colors:
   - Key: GNOME blue (#3584e4)
   - Tunnel: Darker blue/gray (#1c71d8)
   - Gradient for depth
```

## Design Decision Needed

**User Input Required:** Which icon concept do you prefer?
1. Tunnel + Network
2. Lock + Arrows
3. Terminal + Shield
4. Key + Tunnel (recommended)
5. Other idea?

Once decided, we can create the actual SVG icon using Inkscape.
