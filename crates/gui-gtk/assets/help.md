# SSH Tunnel Manager - Help

## Overview

SSH Tunnel Manager is a GUI application for managing SSH tunnels through a background daemon. It allows you to create, start, stop, and monitor SSH port forwarding connections.

## Getting Started

### Starting the Daemon

The daemon must be running before using the GUI:

```bash
ssh-tunnel-daemon
```

The daemon status icon in the top-right shows the connection state:
- **Green**: Connected to daemon
- **Pulsing**: Connecting to daemon
- **Red**: Daemon offline

## Managing Profiles

### Creating a Profile

1. Click the **+** button in the profiles list
2. Fill in the connection details:
   - **Name**: A friendly name for the profile
   - **SSH Host**: The remote server address
   - **SSH Port**: Usually 22
   - **Username**: SSH username
   - **Authentication**: Choose Password, SSH Key, or Password + 2FA

3. Configure port forwarding:
   - **Local Port**: Port on your machine
   - **Remote Host**: Destination host (often localhost)
   - **Remote Port**: Destination port
   - **Bind Address**: Usually 127.0.0.1

### Starting a Tunnel

1. Select a profile from the list
2. Click the **Start** button
3. If authentication is required, enter your password or 2FA code

### Profile Status Indicators

Each profile shows a colored dot indicating its status:
- **Gray** ●: Not connected
- **Green** ●: Connected and active
- **Orange** ● (pulsing): Connecting or transitioning
- **Red** ●: Failed connection

## Keyboard Shortcuts

- **Ctrl+Q**: Quit application
- **Escape**: Go back / close dialogs

## Troubleshooting

### Daemon Won't Connect

- Ensure the daemon is running: `ssh-tunnel-daemon`
- Check daemon logs for errors
- Verify the daemon socket exists: `/run/user/$(id -u)/ssh-tunnel-manager/ssh-tunnel-manager.sock`

### Tunnel Fails to Connect

- Verify SSH credentials are correct
- Check that the SSH server is reachable
- Ensure the remote port is not already in use
- Check daemon logs for detailed error messages

### Authentication Issues

- For SSH keys, ensure the key file has correct permissions (600)
- For 2FA, ensure you're entering the code within the time window
- Stored passphrases are saved in the system keyring

## Configuration Files

- **Profiles**: `~/.config/ssh-tunnel-manager/profiles/*.toml`
- **Daemon config**: `~/.config/ssh-tunnel-manager/daemon.toml`
- **Known hosts**: `~/.config/ssh-tunnel-manager/known_hosts`

## More Information

For more details, visit the project repository or consult the daemon documentation.
