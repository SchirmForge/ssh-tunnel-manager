Name: ssh-tunnel-manager
Version: 0.1.7
Release: 1%{?dist}
Summary: SSH tunnel manager daemon, CLI, and GUI
License: Apache-2.0
URL: https://github.com/SchirmForge/ssh-tunnel
Source0: %{name}-%{version}.tar.gz

BuildRequires: cargo
BuildRequires: rust
BuildRequires: pkgconfig
BuildRequires: gtk4-devel
BuildRequires: libadwaita-devel
BuildRequires: glib2-devel
BuildRequires: libsecret-devel
BuildRequires: systemd-rpm-macros

%description
SSH Tunnel Manager provides a daemon, CLI, and GUI for managing SSH tunnels.

%package -n ssh-tunnel-daemon
Summary: SSH Tunnel Manager daemon
Suggests: ssh-tunnel-cli

%description -n ssh-tunnel-daemon
The daemon manages SSH tunnels and exposes an API for clients.

%package -n ssh-tunnel-cli
Summary: SSH Tunnel Manager CLI
Suggests: ssh-tunnel-daemon

%description -n ssh-tunnel-cli
Command-line interface to manage SSH tunnel profiles and sessions.

%package -n ssh-tunnel-gui
Summary: SSH Tunnel Manager GUI
Suggests: ssh-tunnel-daemon, ssh-tunnel-cli
Supplements: (gnome-shell or plasma-workspace) and ssh-tunnel-daemon

%description -n ssh-tunnel-gui
GTK4-based GUI for managing SSH tunnels.

%prep
%autosetup -n %{name}-%{version}

%build
cargo build --release --package ssh-tunnel-cli --package ssh-tunnel-daemon --package ssh-tunnel-gui

%install
install -Dm755 target/release/ssh-tunnel-daemon %{buildroot}%{_bindir}/ssh-tunnel-daemon
install -Dm755 target/release/ssh-tunnel %{buildroot}%{_bindir}/ssh-tunnel
install -Dm755 target/release/ssh-tunnel-gui %{buildroot}%{_bindir}/ssh-tunnel-gui
install -Dm644 packaging/systemd/ssh-tunnel-daemon@.service %{buildroot}%{_unitdir}/ssh-tunnel-daemon@.service
install -Dm644 packaging/systemd/ssh-tunnel-daemon.service %{buildroot}%{_userunitdir}/ssh-tunnel-daemon.service
install -Dm644 packaging/desktop/com.github.ssh-tunnel-manager.desktop %{buildroot}%{_datadir}/applications/com.github.ssh-tunnel-manager.desktop
install -Dm644 packaging/appstream/com.github.ssh-tunnel-manager.metainfo.xml %{buildroot}%{_datadir}/metainfo/com.github.ssh-tunnel-manager.metainfo.xml

%files
%license LICENSE
%doc README.md CHANGELOG.md

%files -n ssh-tunnel-daemon
%{_bindir}/ssh-tunnel-daemon
%{_unitdir}/ssh-tunnel-daemon@.service
%{_userunitdir}/ssh-tunnel-daemon.service

%files -n ssh-tunnel-cli
%{_bindir}/ssh-tunnel

%files -n ssh-tunnel-gui
%{_bindir}/ssh-tunnel-gui
%{_datadir}/applications/com.github.ssh-tunnel-manager.desktop
%{_datadir}/metainfo/com.github.ssh-tunnel-manager.metainfo.xml
