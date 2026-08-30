# FreeBSD Porting Guide for SSH Tunnel Manager

> **Status: unscheduled.** This plan is kept for reference but is not part of the current
> cycle. Two notes since it was written: `crates/tray` has been deleted, and `gui-qt` is
> now excluded from `default-members`, which removes some of the conditional-member
> gymnastics in §6.

## Executive Summary

The SSH Tunnel Manager daemon and CLI are **70-80% compatible** with FreeBSD out of the box. The core functionality (SSH tunneling, REST API, SSE) uses pure Rust libraries that are cross-platform. However, there are **3 critical blockers** that prevent compilation and deployment on FreeBSD:

1. **errno handling** - Uses Linux-specific `__errno_location()`
2. **Keyring/Secret Service** - Linux D-Bus dependency
3. **Init system** - systemd-specific service files

The GTK GUI will never work on FreeBSD without major refactoring due to Linux-specific dependencies (D-Bus, FreeDesktop portals).

**Recommended approach:** Port daemon + CLI only, create a simple web UI for management.

---

## Critical Files Requiring Patches

### 1. `crates/daemon/src/pidfile.rs` (Lines 87-115)

**Problem:** Uses Linux-specific `libc::__errno_location()`

**Current code:**
```rust
#[cfg(unix)]
fn is_process_running(pid: u32) -> bool {
    unsafe {
        let result = libc::kill(pid as i32, 0);
        if result == 0 {
            return true;
        }
        let errno = *libc::__errno_location();  // ❌ LINUX-SPECIFIC
        match errno {
            libc::ESRCH => false,
            libc::EPERM => true,
            _ => false,
        }
    }
}
```

**Solution:** Use platform-specific errno handling

**Patched code:**
```rust
#[cfg(unix)]
fn is_process_running(pid: u32) -> bool {
    unsafe {
        let result = libc::kill(pid as i32, 0);
        if result == 0 {
            return true;
        }

        #[cfg(target_os = "linux")]
        let errno = *libc::__errno_location();

        #[cfg(any(target_os = "freebsd", target_os = "openbsd", target_os = "netbsd"))]
        let errno = *libc::__error();

        #[cfg(target_os = "macos")]
        let errno = *libc::__error();

        match errno {
            libc::ESRCH => false,  // No such process
            libc::EPERM => true,   // Process exists but no permission
            _ => false,
        }
    }
}
```

**Alternative solution (using errno crate):**
```rust
// Add to Cargo.toml:
// errno = "0.3"

#[cfg(unix)]
fn is_process_running(pid: u32) -> bool {
    unsafe {
        let result = libc::kill(pid as i32, 0);
        if result == 0 {
            return true;
        }

        let errno = errno::errno().0;
        match errno {
            libc::ESRCH => false,
            libc::EPERM => true,
            _ => false,
        }
    }
}
```

---

### 2. Workspace `Cargo.toml` (Line 50)

**Problem:** `secret-service` crate is Linux D-Bus specific and cannot be used on FreeBSD

**Current code:**
```toml
# Security
secret-service = { version = "4.0", features = ["rt-tokio-crypto-rust"] }
```

**Solution:** Make it Linux-only with optional feature flag

**Patched code:**
```toml
# Security
[target.'cfg(target_os = "linux")'.dependencies]
secret-service = { version = "4.0", features = ["rt-tokio-crypto-rust"] }
```

---

### 3. `crates/daemon/Cargo.toml` (Line 51)

**Problem:** Daemon directly depends on `secret-service`

**Current code:**
```toml
# Security
secret-service = { workspace = true }
```

**Solution:** Make it conditional

**Patched code:**
```toml
# Security (Linux only)
[target.'cfg(target_os = "linux")'.dependencies]
secret-service = { workspace = true }
```

---

### 4. `crates/common/Cargo.toml` (Line 48)

**Problem:** `keyring` crate uses Linux-specific features, no BSD support

**Current code:**
```toml
keyring = { version = "3.6", features = ["linux-native", "apple-native", "windows-native"] }
```

**Solution:** Add platform-specific feature selection or make keyring optional

**Option A - Platform-specific features:**
```toml
[target.'cfg(target_os = "linux")'.dependencies]
keyring = { version = "3.6", features = ["linux-native"] }

[target.'cfg(target_os = "macos")'.dependencies]
keyring = { version = "3.6", features = ["apple-native"] }

[target.'cfg(target_os = "windows")'.dependencies]
keyring = { version = "3.6", features = ["windows-native"] }

[target.'cfg(target_os = "freebsd")'.dependencies]
keyring = { version = "3.6", default-features = false }
```

**Option B - Make keyring optional:**
```toml
[dependencies]
keyring = { version = "3.6", optional = true }

[features]
default = ["keyring-support"]
keyring-support = ["keyring"]
```

Then update `crates/common/src/keychain.rs` to have feature-gated implementations.

---

### 5. `crates/common/src/keychain.rs` (Entire file)

**Problem:** Assumes keyring is always available

**Solution:** Add FreeBSD-specific fallback or feature-gating

**Patched code (add conditional compilation):**
```rust
#[cfg(not(target_os = "freebsd"))]
use keyring::Entry;

pub fn store_password(profile_id: &Uuid, password: &str) -> Result<()> {
    #[cfg(not(target_os = "freebsd"))]
    {
        let entry = Entry::new("ssh-tunnel-manager", &profile_id.to_string())
            .map_err(|e| Error::Keychain(format!("Failed to create keychain entry: {}", e)))?;

        entry
            .set_password(password)
            .map_err(|e| Error::Keychain(format!("Failed to store password in keychain: {}", e)))?;

        Ok(())
    }

    #[cfg(target_os = "freebsd")]
    {
        Err(Error::Keychain(
            "Keychain support not available on FreeBSD. Use config-based auth.".to_string()
        ))
    }
}

pub fn get_password(profile_id: &Uuid) -> Result<String> {
    #[cfg(not(target_os = "freebsd"))]
    {
        let entry = Entry::new("ssh-tunnel-manager", &profile_id.to_string())
            .map_err(|e| Error::Keychain(format!("Failed to access keychain entry: {}", e)))?;

        entry
            .get_password()
            .map_err(|e| Error::Keychain(format!("Failed to retrieve password from keychain: {}", e)))
    }

    #[cfg(target_os = "freebsd")]
    {
        Err(Error::Keychain(
            "Keychain support not available on FreeBSD. Use config-based auth.".to_string()
        ))
    }
}

// Similar updates for remove_password() and has_password()
```

---

### 6. Disable GTK GUI and Tray for FreeBSD

**Problem:** GTK GUI depends on Linux-specific D-Bus services (ashpd, ksni, notify-rust)

**Workspace `Cargo.toml` changes:**

**Current members:**
```toml
members = [
    "crates/daemon",
    "crates/cli",
    "crates/gui-gtk",
    "crates/gui-qt",
    "crates/gui-core",
    "crates/common",
]
```

**Solution:** Use conditional compilation for GUI crates

**Patched code:**
```toml
members = [
    "crates/daemon",
    "crates/cli",
    "crates/common",
]

[target.'cfg(target_os = "linux")'.workspace.members]
additional-members = [
    "crates/gui-gtk",
    "crates/gui-qt",
    "crates/gui-core",
]
```

**Note:** Cargo doesn't support conditional workspace members directly. Instead, use build profile or feature flags:

**Better solution - Add feature flags:**
```toml
[features]
default = ["gui"]
gui = []

# Then in crates/gui-gtk/Cargo.toml:
[features]
default = []

# Only build GUI on Linux
[target.'cfg(not(target_os = "linux"))'.dependencies]
# Empty - GUI only builds on Linux
```

---

## FreeBSD-Specific Additions

### 7. Create rc.d Service Script

**File:** `freebsd/etc/rc.d/ssh_tunnel_daemon`

```sh
#!/bin/sh

# PROVIDE: ssh_tunnel_daemon
# REQUIRE: NETWORKING DAEMON LOGIN
# BEFORE: LOGIN
# KEYWORD: shutdown

. /etc/rc.subr

name="ssh_tunnel_daemon"
rcvar="${name}_enable"
desc="SSH Tunnel Manager Daemon"

: ${ssh_tunnel_daemon_enable:="NO"}
: ${ssh_tunnel_daemon_user:=""}
: ${ssh_tunnel_daemon_config:=""}
: ${ssh_tunnel_daemon_flags:=""}

command="/usr/local/bin/ssh-tunnel-daemon"
command_args="${ssh_tunnel_daemon_flags}"
pidfile="/var/run/${name}/${name}.pid"
required_dirs="/var/run/${name}"

# Set up environment
if [ -n "${ssh_tunnel_daemon_config}" ]; then
    command_args="${command_args} --config ${ssh_tunnel_daemon_config}"
fi

# Set RUST_LOG if not already set
: ${RUST_LOG:="info"}
export RUST_LOG

# Run as specific user if configured
if [ -n "${ssh_tunnel_daemon_user}" ]; then
    command_user="${ssh_tunnel_daemon_user}"
fi

# Create runtime directory
start_precmd="ssh_tunnel_daemon_prestart"

ssh_tunnel_daemon_prestart()
{
    if [ ! -d "/var/run/${name}" ]; then
        install -d -o ${command_user:-root} -g wheel -m 0770 "/var/run/${name}"
    fi

    # Create config directory if running as specific user
    if [ -n "${command_user}" ]; then
        user_home=$(eval echo ~${command_user})
        config_dir="${user_home}/.config/ssh-tunnel-manager"
        if [ ! -d "${config_dir}" ]; then
            install -d -o ${command_user} -m 0700 "${config_dir}"
        fi
    fi
}

load_rc_config $name
run_rc_command "$@"
```

**Configuration file:** `freebsd/etc/rc.conf.d/ssh_tunnel_daemon`

```sh
# Enable the service
ssh_tunnel_daemon_enable="YES"

# Run as specific user (optional, defaults to root)
ssh_tunnel_daemon_user="your_username"

# Custom config file (optional)
#ssh_tunnel_daemon_config="/usr/local/etc/ssh-tunnel-manager/daemon.toml"

# Additional flags (optional)
#ssh_tunnel_daemon_flags=""

# Logging level
RUST_LOG="info"
```

---

### 8. FreeBSD Port Structure

**Directory:** `/usr/ports/net/ssh-tunnel-manager/`

**Makefile:**
```makefile
PORTNAME=       ssh-tunnel-manager
DISTVERSION=    0.1.10
CATEGORIES=     net security
MASTER_SITES=   https://github.com/SchirmForge/ssh-tunnel/archive/
DISTNAME=       v${DISTVERSION}

MAINTAINER=     your-email@example.com
COMMENT=        SSH tunnel manager with REST API and web UI
WWW=            https://github.com/SchirmForge/ssh-tunnel

LICENSE=        APACHE20
LICENSE_FILE=   ${WRKSRC}/LICENSE

USES=           cargo ssl
USE_GITHUB=     yes
GH_ACCOUNT=     SchirmForge
GH_PROJECT=     ssh-tunnel

CARGO_CRATES=   # Auto-generated list from Cargo.lock

PLIST_FILES=    bin/ssh-tunnel-daemon \
                bin/ssh-tunnel-cli \
                etc/rc.d/ssh_tunnel_daemon

USE_RC_SUBR=    ssh_tunnel_daemon

CARGO_BUILD_ARGS=   --package ssh-tunnel-daemon --package ssh-tunnel-cli

post-install:
        ${STRIP_CMD} ${STAGEDIR}${PREFIX}/bin/ssh-tunnel-daemon
        ${STRIP_CMD} ${STAGEDIR}${PREFIX}/bin/ssh-tunnel-cli
        ${INSTALL_SCRIPT} ${WRKSRC}/freebsd/etc/rc.d/ssh_tunnel_daemon \
                ${STAGEDIR}${PREFIX}/etc/rc.d/

.include <bsd.port.mk>
```

**pkg-descr:**
```
SSH Tunnel Manager is a modern SSH tunnel management tool with:
- Background daemon for managing SSH tunnels
- REST API for programmatic control
- Command-line interface for interactive use
- Real-time status updates via Server-Sent Events
- Support for multiple authentication methods
- Automatic reconnection and health monitoring

This port includes only the daemon and CLI components.
A web-based UI can be added separately.

WWW: https://github.com/SchirmForge/ssh-tunnel
```

---

### 9. Build Instructions for FreeBSD

**Manual build:**
```bash
# Install dependencies
pkg install rust pkgconf git

# Clone repository
git clone https://github.com/SchirmForge/ssh-tunnel.git
cd ssh-tunnel

# Apply FreeBSD patches (create these patches from above changes)
patch -p1 < freebsd/patches/001-errno-handling.patch
patch -p1 < freebsd/patches/002-disable-secret-service.patch
patch -p1 < freebsd/patches/003-keyring-optional.patch

# Build daemon and CLI only
cargo build --release --package ssh-tunnel-daemon --package ssh-tunnel-cli

# Install binaries
install -m 755 target/release/ssh-tunnel-daemon /usr/local/bin/
install -m 755 target/release/ssh-tunnel-cli /usr/local/bin/

# Install rc.d script
install -m 755 freebsd/etc/rc.d/ssh_tunnel_daemon /usr/local/etc/rc.d/

# Enable and start service
sysrc ssh_tunnel_daemon_enable="YES"
sysrc ssh_tunnel_daemon_user="$(whoami)"
service ssh_tunnel_daemon start
```

---

## Web UI Integration for OPNSense

Since the GTK GUI won't work on FreeBSD, a web UI is the recommended approach.

### Architecture

```
┌─────────────────────────────────────┐
│  OPNSense/pfSense (FreeBSD)         │
│                                     │
│  ┌───────────────────────────────┐ │
│  │ ssh-tunnel-daemon             │ │
│  │ - Runs as rc.d service        │ │
│  │ - Binds to 127.0.0.1:8443     │ │
│  │ - REST API + SSE events       │ │
│  └───────────────▲────────────────┘ │
│                  │                  │
│  ┌───────────────┴────────────────┐ │
│  │ nginx (reverse proxy)          │ │
│  │ - Serves static web UI         │ │
│  │ - Proxies /api/* to daemon     │ │
│  │ - Port 443 HTTPS               │ │
│  └────────────────────────────────┘ │
│                                     │
└─────────────────────────────────────┘
           │
           │ HTTPS
           │
    ┌──────▼──────┐
    │   Browser   │
    │  (Web UI)   │
    └─────────────┘
```

### Required Changes to Daemon

**1. Add CORS support** (for browser-based UI)

**File:** `crates/daemon/src/api.rs`

**Add to Cargo.toml:**
```toml
tower-http = { workspace = true, features = ["cors"] }
```

**Modify `create_router()` function (around line 113):**
```rust
use tower_http::cors::{CorsLayer, Any};

pub fn create_router(
    tunnel_manager: Arc<TunnelManager>,
    event_bus: Arc<EventBus>,
) -> Router {
    Router::new()
        // ... existing routes ...
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods([Method::GET, Method::POST])
                .allow_headers([header::CONTENT_TYPE, HeaderName::from_static("x-tunnel-token")])
        )
        .layer(TraceLayer::new_for_http())
}
```

**2. Optional: Add static file serving**

```rust
use tower_http::services::ServeDir;

pub fn create_router(
    tunnel_manager: Arc<TunnelManager>,
    event_bus: Arc<EventBus>,
    static_dir: Option<PathBuf>,
) -> Router {
    let mut router = Router::new()
        // ... API routes under /api/ ...

    if let Some(dir) = static_dir {
        router = router.nest_service("/", ServeDir::new(dir));
    }

    router
        .layer(cors_layer)
        .layer(trace_layer)
}
```

### Minimal Web UI Implementation

**File:** `web-ui/index.html` (single-page app)

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>SSH Tunnel Manager</title>
    <script src="https://cdn.jsdelivr.net/npm/vue@3/dist/vue.global.prod.js"></script>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body {
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
            background: #f5f5f5;
            padding: 20px;
        }
        .container { max-width: 1200px; margin: 0 auto; }
        h1 { margin-bottom: 20px; color: #333; }
        .tunnel-card {
            background: white;
            border-radius: 8px;
            padding: 20px;
            margin-bottom: 15px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
        }
        .tunnel-header { display: flex; justify-content: space-between; align-items: center; }
        .status {
            padding: 4px 12px;
            border-radius: 12px;
            font-size: 12px;
            font-weight: 600;
        }
        .status.connected { background: #d4edda; color: #155724; }
        .status.disconnected { background: #f8d7da; color: #721c24; }
        .status.connecting { background: #fff3cd; color: #856404; }
        button {
            padding: 8px 16px;
            border: none;
            border-radius: 4px;
            cursor: pointer;
            font-weight: 500;
            margin-left: 8px;
        }
        button.start { background: #28a745; color: white; }
        button.stop { background: #dc3545; color: white; }
        button:hover { opacity: 0.9; }
        button:disabled { opacity: 0.5; cursor: not-allowed; }
        .auth-prompt {
            position: fixed;
            top: 50%;
            left: 50%;
            transform: translate(-50%, -50%);
            background: white;
            padding: 30px;
            border-radius: 8px;
            box-shadow: 0 4px 20px rgba(0,0,0,0.3);
            z-index: 1000;
            min-width: 400px;
        }
        .overlay {
            position: fixed;
            top: 0;
            left: 0;
            width: 100%;
            height: 100%;
            background: rgba(0,0,0,0.5);
            z-index: 999;
        }
        input[type="password"], input[type="text"] {
            width: 100%;
            padding: 10px;
            margin: 10px 0;
            border: 1px solid #ddd;
            border-radius: 4px;
        }
    </style>
</head>
<body>
    <div id="app">
        <div class="container">
            <h1>SSH Tunnel Manager</h1>

            <div v-if="tunnels.length === 0" style="text-align: center; padding: 40px; color: #999;">
                No active tunnels
            </div>

            <div v-for="tunnel in tunnels" :key="tunnel.id" class="tunnel-card">
                <div class="tunnel-header">
                    <div>
                        <h3>{{ tunnel.name || tunnel.id }}</h3>
                        <span :class="'status ' + tunnel.status">{{ tunnel.status }}</span>
                    </div>
                    <div>
                        <button v-if="tunnel.status === 'disconnected'"
                                @click="startTunnel(tunnel.id)"
                                class="start">
                            Start
                        </button>
                        <button v-if="tunnel.status === 'connected'"
                                @click="stopTunnel(tunnel.id)"
                                class="stop">
                            Stop
                        </button>
                    </div>
                </div>
            </div>
        </div>

        <!-- Auth prompt dialog -->
        <div v-if="authPrompt" class="overlay" @click="cancelAuth"></div>
        <div v-if="authPrompt" class="auth-prompt">
            <h3>{{ authPrompt.prompt }}</h3>
            <input
                :type="authPrompt.hidden ? 'password' : 'text'"
                v-model="authResponse"
                @keyup.enter="submitAuth"
                autofocus
            />
            <div style="text-align: right; margin-top: 15px;">
                <button @click="cancelAuth" style="background: #6c757d; color: white;">Cancel</button>
                <button @click="submitAuth" style="background: #007bff; color: white;">Submit</button>
            </div>
        </div>
    </div>

    <script>
        const { createApp } = Vue;

        const API_BASE = window.location.origin + '/api';
        const TOKEN = localStorage.getItem('tunnel-token') || prompt('Enter daemon token:');
        if (TOKEN) localStorage.setItem('tunnel-token', TOKEN);

        createApp({
            data() {
                return {
                    tunnels: [],
                    authPrompt: null,
                    authResponse: '',
                    eventSource: null
                };
            },

            async mounted() {
                await this.loadTunnels();
                this.connectSSE();
            },

            methods: {
                async apiCall(endpoint, options = {}) {
                    const headers = {
                        'Content-Type': 'application/json',
                        'X-Tunnel-Token': TOKEN,
                        ...options.headers
                    };

                    const response = await fetch(API_BASE + endpoint, {
                        ...options,
                        headers
                    });

                    if (!response.ok) {
                        throw new Error(`API error: ${response.status}`);
                    }

                    return response.json().catch(() => null);
                },

                async loadTunnels() {
                    try {
                        const data = await this.apiCall('/tunnels');
                        this.tunnels = data.tunnels || [];
                    } catch (error) {
                        console.error('Failed to load tunnels:', error);
                        alert('Failed to connect to daemon. Check token and connection.');
                    }
                },

                async startTunnel(id) {
                    try {
                        await this.apiCall(`/tunnels/${id}/start`, { method: 'POST' });
                    } catch (error) {
                        console.error('Failed to start tunnel:', error);
                    }
                },

                async stopTunnel(id) {
                    try {
                        await this.apiCall(`/tunnels/${id}/stop`, { method: 'POST' });
                    } catch (error) {
                        console.error('Failed to stop tunnel:', error);
                    }
                },

                connectSSE() {
                    this.eventSource = new EventSource(API_BASE + '/events');

                    this.eventSource.onmessage = (event) => {
                        const data = JSON.parse(event.data);

                        if (data.connected) {
                            this.updateTunnelStatus(data.connected.id, 'connected');
                        } else if (data.disconnected) {
                            this.updateTunnelStatus(data.disconnected.id, 'disconnected');
                        } else if (data.starting) {
                            this.updateTunnelStatus(data.starting.id, 'connecting');
                        } else if (data.auth_required) {
                            this.authPrompt = data.auth_required.request;
                        }
                    };

                    this.eventSource.onerror = () => {
                        console.log('SSE connection lost, reconnecting...');
                        setTimeout(() => this.connectSSE(), 5000);
                    };
                },

                updateTunnelStatus(id, status) {
                    const tunnel = this.tunnels.find(t => t.id === id);
                    if (tunnel) {
                        tunnel.status = status;
                    }
                },

                async submitAuth() {
                    if (!this.authPrompt) return;

                    try {
                        await this.apiCall(`/tunnels/${this.authPrompt.tunnel_id}/auth`, {
                            method: 'POST',
                            body: JSON.stringify({
                                request_id: this.authPrompt.id,
                                response: this.authResponse
                            })
                        });
                    } catch (error) {
                        console.error('Failed to submit auth:', error);
                    }

                    this.authPrompt = null;
                    this.authResponse = '';
                },

                cancelAuth() {
                    this.authPrompt = null;
                    this.authResponse = '';
                }
            }
        }).mount('#app');
    </script>
</body>
</html>
```

### nginx Configuration

**File:** `/usr/local/etc/nginx/conf.d/ssh-tunnel-manager.conf`

```nginx
server {
    listen 443 ssl;
    server_name tunnel.example.com;

    ssl_certificate /usr/local/etc/ssl/certs/tunnel.crt;
    ssl_certificate_key /usr/local/etc/ssl/private/tunnel.key;

    # Serve web UI
    location / {
        root /usr/local/www/ssh-tunnel-ui;
        index index.html;
        try_files $uri $uri/ /index.html;
    }

    # Proxy API requests to daemon
    location /api/ {
        proxy_pass http://127.0.0.1:8443;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;

        # Pass through auth token
        proxy_set_header X-Tunnel-Token $http_x_tunnel_token;

        # SSE support
        proxy_buffering off;
        proxy_cache off;
        proxy_set_header Connection '';
        chunked_transfer_encoding off;
    }
}
```

---

## Testing Checklist

### Phase 1: Build Verification
- [ ] Apply all patches from this guide
- [ ] Run `cargo check --package ssh-tunnel-daemon`
- [ ] Run `cargo check --package ssh-tunnel-cli`
- [ ] Run `cargo build --release --package ssh-tunnel-daemon --package ssh-tunnel-cli`
- [ ] Verify binaries exist in `target/release/`

### Phase 2: Daemon Functionality
- [ ] Create minimal config file
- [ ] Start daemon manually: `./target/release/ssh-tunnel-daemon`
- [ ] Verify daemon binds to port (check with `sockstat -l`)
- [ ] Test health endpoint: `curl http://localhost:8443/api/health`
- [ ] Test daemon info: `curl http://localhost:8443/api/daemon/info`
- [ ] Create a test tunnel profile
- [ ] Start tunnel via API
- [ ] Verify SSH connection works
- [ ] Stop daemon gracefully (Ctrl+C)

### Phase 3: rc.d Service
- [ ] Install rc.d script
- [ ] Configure in `/etc/rc.conf`
- [ ] Start service: `service ssh_tunnel_daemon start`
- [ ] Check service status: `service ssh_tunnel_daemon status`
- [ ] Verify PID file created
- [ ] Test service restart
- [ ] Test service stop
- [ ] Verify PID file removed on stop

### Phase 4: Web UI
- [ ] Deploy web UI files
- [ ] Configure nginx reverse proxy
- [ ] Test web UI loads in browser
- [ ] Test API calls from browser
- [ ] Test SSE event stream
- [ ] Test tunnel start/stop
- [ ] Test auth prompt dialog

---

## Deployment Workflow for OPNSense

### Step 1: Install Daemon on OPNSense

```bash
# SSH into OPNSense
ssh root@opnsense.local

# Install dependencies
pkg install rust pkgconf git nginx

# Build and install daemon
cd /tmp
git clone https://github.com/SchirmForge/ssh-tunnel.git
cd ssh-tunnel

# Apply FreeBSD patches
patch -p1 < freebsd/patches/all-in-one.patch

# Build
cargo build --release --package ssh-tunnel-daemon --package ssh-tunnel-cli

# Install
install -m 755 target/release/ssh-tunnel-daemon /usr/local/bin/
install -m 755 target/release/ssh-tunnel-cli /usr/local/bin/
install -m 755 freebsd/etc/rc.d/ssh_tunnel_daemon /usr/local/etc/rc.d/

# Configure daemon
mkdir -p ~/.config/ssh-tunnel-manager
cat > ~/.config/ssh-tunnel-manager/daemon.toml <<EOF
host = "127.0.0.1"
port = 8443
tls_mode = "disabled"  # nginx will handle TLS
require_auth = true
group_access = false
EOF

# Enable and start
sysrc ssh_tunnel_daemon_enable="YES"
service ssh_tunnel_daemon start
```

### Step 2: Deploy Web UI

```bash
# Create web UI directory
mkdir -p /usr/local/www/ssh-tunnel-ui

# Copy web UI files
cp web-ui/index.html /usr/local/www/ssh-tunnel-ui/

# Configure nginx
cat > /usr/local/etc/nginx/conf.d/ssh-tunnel.conf <<EOF
# ... nginx config from above ...
EOF

# Test nginx config
nginx -t

# Reload nginx
service nginx reload
```

### Step 3: Configure Firewall

```bash
# In OPNSense web UI, add firewall rule:
# - Interface: LAN
# - Protocol: TCP
# - Port: 443
# - Destination: This firewall
# - Description: SSH Tunnel Manager Web UI
```

### Step 4: Access Web UI

1. Open browser to `https://opnsense.local`
2. Enter daemon token (found in `~/.config/ssh-tunnel-manager/daemon.token`)
3. Manage tunnels via web interface

---

## Summary

**Files to create:**
1. `freebsd/patches/001-errno-handling.patch`
2. `freebsd/patches/002-disable-secret-service.patch`
3. `freebsd/patches/003-keyring-optional.patch`
4. `freebsd/etc/rc.d/ssh_tunnel_daemon`
5. `freebsd/etc/rc.conf.d/ssh_tunnel_daemon`
6. `ports/net/ssh-tunnel-manager/Makefile`
7. `ports/net/ssh-tunnel-manager/pkg-descr`
8. `web-ui/index.html`

**Effort estimate:**
- Code patches: 2-4 hours
- rc.d scripts: 2-3 hours
- FreeBSD port: 4-6 hours
- Web UI development: 8-12 hours
- Testing and documentation: 4-6 hours

**Total:** 20-31 hours for complete FreeBSD port with web UI

**Recommended approach:**
1. Start with code patches (fix errno, disable GUI, optional keyring)
2. Test manual build and execution
3. Create rc.d service script
4. Build minimal web UI
5. Package as FreeBSD port

This guide provides everything needed to port SSH Tunnel Manager to FreeBSD and deploy it on OPNSense/pfSense firewalls with a web-based management interface.
