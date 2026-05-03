Name:           cardwire
Version:        0.6.0
Release:        1%{?dist}
Summary:        A GPU manager for Linux using eBPF LSM hooks
License:        GPL-3.0
URL:            https://github.com/OpenGamingCollective/cardwire
Source0:        %{url}/archive/v%{version}/%{name}-%{version}.tar.gz

BuildRequires:  rust
BuildRequires:  cargo
BuildRequires:  clang
BuildRequires:  libbpf-devel
BuildRequires:  make
BuildRequires:  systemd-rpm-macros

Requires: hwdata
Requires: upower

%description
Cardwire is a GPU manager for Linux that uses eBPF LSM hooks to block or
unblock access to GPU device nodes.

%prep
%autosetup -n %{name}-%{version}
mkdir -p .cargo
cat > .cargo/config.toml << 'EOF'
[net]
offline = false
EOF

%build
/usr/bin/cargo build --release --locked

%install
%define _target_dir target/release

# Install binaries
install -D -m 0755 %{_target_dir}/cardwire %{buildroot}%{_bindir}/cardwire
install -D -m 0755 %{_target_dir}/cardwired %{buildroot}%{_bindir}/cardwired

# Install systemd unit
install -D -m 0644 assets/cardwired.service %{buildroot}%{_unitdir}/cardwired.service

# Install systemd preset
install -D -m 0644 assets/99-cardwired.preset %{buildroot}%{_presetdir}/99-cardwired.preset

# Install D-Bus system policy
install -D -m 0644 assets/com.github.opengamingcollective.cardwire.conf %{buildroot}%{_datadir}/dbus-1/system.d/com.github.opengamingcollective.cardwire.conf

%post
%systemd_post cardwired.service

%preun
%systemd_preun cardwired.service

%postun
%systemd_postun_with_restart cardwired.service

%files
%license LICENSE
%doc README.md
%{_bindir}/cardwire
%{_bindir}/cardwired
%{_unitdir}/cardwired.service
%{_presetdir}/99-cardwired.preset
%{_datadir}/dbus-1/system.d/com.github.opengamingcollective.cardwire.conf

%changelog
* Mon Apr 27 2026 luytan <luytan@khora.me> - 0.4.1-1
- Added preset and removed useless lines
* Mon Apr 27 2026 luytan <luytan@khora.me> - 0.4.1-1
- Initial package
