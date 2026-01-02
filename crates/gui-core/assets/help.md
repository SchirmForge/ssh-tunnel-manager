# SSH Tunnel Manager - Help

## Quick Start

### First Launch

When you launch SSH Tunnel Manager for the first time, the **Configuration Wizard** will help you set up your daemon connection:

1. **Automatic Detection** (Recommended):
   - If the daemon is running locally and has generated a config snippet, click "Yes" to import it automatically
   - The wizard will configure your connection settings

2. **Manual Configuration**:
   - If automatic detection isn't available or you're connecting to a remote daemon, enter:
     - **Connection Mode**: Unix Socket (local) or HTTPS (remote)
     - **Daemon Host**: IP address or hostname (for HTTPS mode)
     - **Daemon Port**: Usually 3443 for HTTPS
     - **Auth Token**: Copy from daemon's auth-token file
     - **TLS Fingerprint**: Copy from daemon's config snippet

3. **Start Using**:
   - Once configured, the GUI will connect to the daemon
   - You can now create profiles and start tunnels

### Daemon Status

The daemon status indicator in the top-right shows:
- **Green dot**: Connected to daemon
- **Orange dot (pulsing)**: Connecting to daemon
- **Red dot**: Daemon offline
- **Gray dot**: Not configured

## Managing Profiles

### Creating a Profile

1. Click the **+** button in the toolbar
2. Fill in the connection details:

   **SSH Connection:**
   - **Profile Name**: Friendly name for this tunnel
   - **SSH Host**: Remote server address (e.g., `ssh.example.com`)
   - **SSH Port**: Usually `22`
   - **Username**: Your SSH username

   **Authentication:**
   - **SSH Key**: Path to private key file (e.g., `~/.ssh/id_ed25519`)
   - **Password**: Choose if using password authentication
   - **Store in Keychain**: Save passphrase securely in system keyring

3. Configure port forwarding:

   **Port Forwarding:**
   - **Local Port**: Port on your machine (e.g., `8080`)
   - **Remote Host**: Destination host, often `localhost` on the SSH server
   - **Remote Port**: Destination port (e.g., `80` for a web server)
   - **Bind Address**: Usually `127.0.0.1` (localhost only)

4. Click **Save** to create the profile

### Editing a Profile

- Select a profile and click the **Edit** button (pencil icon)
- Modify any settings and click **Save**
- Note: You must stop an active tunnel before editing its profile

### Starting a Tunnel

1. Select a profile from the list
2. Click the **▶ Start** button
3. If authentication is required:
   - Enter your SSH key passphrase
   - Enter your password
   - Enter 2FA code if prompted

4. The status indicator will show:
   - **Orange (pulsing)**: Connecting
   - **Green**: Connected and active
   - **Red**: Connection failed (check details view for error)

### Stopping a Tunnel

- Click the **■ Stop** button for an active tunnel
- The tunnel will disconnect and the status will return to gray

### Profile Status Indicators

Each profile shows a colored dot:
- **Gray** ●: Not running
- **Green** ●: Connected and active
- **Orange** ● (pulsing): Connecting or transitioning
- **Red** ●: Failed (hover for error details)

## Remote Daemon Support (v0.1.9)

### Connecting to a Remote Daemon

SSH Tunnel Manager can connect to daemons running on other machines via HTTPS:

**On the Remote Server:**
1. Install and configure daemon in HTTPS mode:
   ```bash
   # Edit daemon config
   nano ~/.config/ssh-tunnel-manager/daemon.toml
   # Set: listener_mode = "tcp-https"
   # Set: bind_host = "0.0.0.0"

   # Start daemon
   ssh-tunnel-daemon
   ```

2. Copy the configuration snippet:
   ```bash
   cat ~/.config/ssh-tunnel-manager/cli-config.snippet
   ```

**On Your Local Machine:**
1. Launch SSH Tunnel Manager
2. Use the Configuration Wizard with "Manual Configuration"
3. Enter the remote daemon's details from the snippet
4. Save and connect

### SSH Keys for Remote Daemons

**Important**: SSH private keys must exist on the daemon host filesystem, not on your local machine.

When creating profiles for remote daemons:
1. Copy your SSH keys to the daemon host:
   ```bash
   scp ~/.ssh/id_ed25519 daemon-host:.ssh/
   ```

2. Set correct permissions on daemon host:
   ```bash
   ssh daemon-host "chmod 600 ~/.ssh/id_ed25519"
   ```

3. In the profile, specify just the key filename: `id_ed25519`
   (The daemon will look in `~/.ssh/` on its host)

The GUI will show a warning dialog when starting remote tunnels with SSH key authentication, reminding you to ensure keys are on the daemon host.

## Keyboard Shortcuts

- **Ctrl+Q**: Quit application
- **Escape**: Go back / close dialogs
- **Ctrl+N**: Create new profile
- **Delete**: Delete selected profile (when focused)

## Troubleshooting

### Daemon Won't Connect

**Problem**: Red status indicator

**Solutions**:
- Ensure the daemon is running:
  ```bash
  systemctl --user status ssh-tunnel-daemon
  ```
- Start the daemon if needed:
  ```bash
  systemctl --user start ssh-tunnel-daemon
  ```
- Check daemon logs:
  ```bash
  journalctl --user -u ssh-tunnel-daemon -f
  ```

### Configuration Wizard Doesn't Find Snippet

**Problem**: "No configuration snippet found"

**Solutions**:
- The daemon hasn't generated a snippet yet (run daemon first)
- Use "Manual Configuration" to enter settings manually
- For remote daemons, always use "Manual Configuration"

### Tunnel Fails to Connect

**Problem**: Red profile status with error message

**Solutions**:
- Verify SSH credentials are correct
- Check that the SSH server is reachable
- Ensure the local port is not already in use
- Check daemon logs for detailed error messages
- For remote daemons, ensure SSH keys are on daemon host

### Authentication Issues

**SSH Key Problems**:
- Ensure the key file has correct permissions: `chmod 600 ~/.ssh/id_ed25519`
- For remote daemons, verify the key exists on daemon host, not local machine
- Check the key path in the profile matches the actual file location

**2FA/Keyboard-Interactive**:
- Ensure you're entering the code within the time window
- Some servers require both key AND password - you'll be prompted for both

**Keychain Issues**:
- If keyring is unavailable (headless server), passphrases won't be stored
- You'll need to enter passphrases each time you start a tunnel
- Set `SSH_TUNNEL_SKIP_KEYRING=1` to disable keyring attempts

### Remote Daemon Connection Fails

**HTTPS/TLS Errors**:
- Verify the TLS fingerprint matches the daemon's certificate
- Check that the auth token is correct
- Ensure firewall allows connections on the daemon port (usually 3443)

**401 Authentication Errors**:
- Verify the auth token is correct
- Check daemon requires authentication: `require_auth = true` in daemon.toml
- Token location: `~/.config/ssh-tunnel-manager/auth-token` on daemon host

## Configuration Files

### Local System
- **Client Config**: `~/.config/ssh-tunnel-manager/cli.toml`
  - Daemon connection settings
  - User preferences

### Daemon System (Local or Remote)
- **Profiles**: `~/.config/ssh-tunnel-manager/profiles/*.toml`
- **Daemon Config**: `~/.config/ssh-tunnel-manager/daemon.toml`
- **Auth Token**: `~/.config/ssh-tunnel-manager/auth-token`
- **TLS Certificate**: `~/.config/ssh-tunnel-manager/daemon-cert.pem`
- **Known Hosts**: `~/.config/ssh-tunnel-manager/known_hosts`

## Advanced Features

### Systemd Service

Run the daemon as a systemd user service for automatic startup:

```bash
# Enable and start
systemctl --user enable --now ssh-tunnel-daemon

# View status
systemctl --user status ssh-tunnel-daemon

# View logs
journalctl --user -u ssh-tunnel-daemon -f
```

### Command-Line Interface

For automation and scripting, use the CLI:

```bash
# List all profiles
ssh-tunnel list

# Start a tunnel
ssh-tunnel start myprofile

# Check status
ssh-tunnel status --all

# Stop a tunnel
ssh-tunnel stop myprofile
```

Run `ssh-tunnel --help` for complete CLI documentation.

## Getting More Help

- **Installation Guide**: See `docs/INSTALLATION.md` in the project repository
- **Architecture**: See `docs/ARCHITECTURE.md` for technical details
- **Project Repository**: https://github.com/SchirmForge/ssh-tunnel-manager
- **Report Issues**: https://github.com/SchirmForge/ssh-tunnel-manager/issues

---

**SSH Tunnel Manager** v0.1.9 | Apache-2.0 License
