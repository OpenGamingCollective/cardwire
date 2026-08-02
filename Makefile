debug ?=
$(info debug is $(debug))

ifdef debug
	release :=
	target :=debug
	extension :=-debug
else
	release :=--release
	target :=release
	extension :=
endif

build:
	cargo build $(release)

install:
	install -Dm0755 target/$(target)/cardwire /usr/bin/cardwire$(extension)
	install -Dm0755 target/$(target)/cardwired /usr/bin/cardwired$(extension)
	install -Dm0755 target/$(target)/cardwire-gui /usr/bin/cardwire-gui$(extension)
	install -Dm0644 assets/cardwired.service /usr/lib/systemd/system/cardwired.service
	install -Dm0644 assets/org.opengamingcollective.cardwire.conf /usr/share/dbus-1/system.d/org.opengamingcollective.cardwire.conf
	install -Dm0644 assets/cardwire-gui.desktop /usr/share/applications/cardwire-gui.desktop
	install -Dm0644 assets/org.opengamingcollective.cardwire.metainfo.xml /usr/share/metainfo/org.opengamingcollective.cardwire.metainfo.xml
	for icon in assets/icons/*.svg; do install -Dm0644 "$$icon" "/usr/share/icons/hicolor/scalable/apps/$$(basename "$$icon")"; done
	systemctl enable cardwired.service

check:
	cargo clippy --all-targets --all-features -- -D warnings

.PHONY: build install check
