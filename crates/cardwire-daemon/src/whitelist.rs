//! An array of str that contains program automatically whitelist by cardwire
pub const ALLOWED_PROGRAMS: &[&str] = &[
    "(udev-worker)",
    "systemd-udevd",
    "pacman",
    "dnf",
    "apt",
    "nix",
    "nix-daemon",
    "virtnodedevd",
];
