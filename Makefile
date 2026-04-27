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
	install -Dm0644 assets/cardwired.service /usr/lib/systemd/system/cardwired.service
	install -Dm0644 assets/com.github.opengamingcollective.cardwire.conf /usr/share/dbus-1/system.d/com.github.opengamingcollective.cardwire.conf
	systemctl enable cardwired.service

check:
	cargo clippy --all-targets --all-features -- -D warnings

.PHONY: build install check