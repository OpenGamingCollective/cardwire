#!/usr/bin/env -S just --justfile

default:
    just --list

[group: 'misc']
build:
    cargo build

[group: 'misc']
fmt:
    cargo fmt --all

[group: 'misc']
fmt-ci:
    cargo fmt --all --check

[group: 'test']
test:
    cargo test --all

[group: 'check']
check:
    cargo check --workspace --exclude cardwire-ebpf

[group: 'lint']
lint:
    cargo clippy --all-targets --all-features -- -D warnings

[group: 'doc']
doc:
    cargo doc --all --open

[group: 'install']
install:
    cargo build --all --release --frozen
    install -Dm0755 target/$(target)/cardwire /usr/bin/cardwire$(extension)
    install -Dm0755 target/$(target)/cardwired /usr/bin/cardwired$(extension)
    install -Dm0755 target/$(target)/cardwire-gui /usr/bin/cardwire-gui$(extension)
    install -Dm0644 assets/cardwired.service /usr/lib/systemd/system/cardwired.service
    install -Dm0644 assets/org.opengamingcollective.cardwire.conf /usr/share/dbus-1/system.d/org.opengamingcollective.cardwire.conf
    install -Dm0644 assets/cardwire-gui.desktop /usr/share/applications/cardwire-gui.desktop
    install -Dm0644 assets/org.opengamingcollective.cardwire.metainfo.xml /usr/share/metainfo/org.opengamingcollective.cardwire.metainfo.xml
    for icon in assets/icons/*.svg; do install -Dm0644 "$$icon" "/usr/share/icons/hicolor/scalable/apps/$$(basename "$$icon")"; done
    systemctl enable cardwired.service
