use anyhow::{Context as _, anyhow};
use aya_build::Toolchain;

fn main() -> anyhow::Result<()> {
    let cargo_metadata::Metadata { packages, .. } = cargo_metadata::MetadataCommand::new()
        .no_deps()
        .exec()
        .context("MetadataCommand::exec")?;
    let ebpf_package = packages
        .into_iter()
        .find(|cargo_metadata::Package { name, .. }| name.as_str() == "cardwire-ebpf")
        .ok_or_else(|| anyhow!("cardwire-ebpf package not found"))?;
    let cargo_metadata::Package {
        name,
        manifest_path,
        ..
    } = ebpf_package;
    let ebpf_package = aya_build::Package {
        name: name.as_str(),
        root_dir: manifest_path
            .parent()
            .ok_or_else(|| anyhow!("no parent for {manifest_path}"))?
            .as_str(),
        ..Default::default()
    };
    // aya-build only routes through rustup when it is on PATH; distro builds
    // (nixpkgs and friends) drive their own toolchain and ignore this pin
    //
    // The pin must pair with the LLVM inside the bpf-linker rustup users have on
    // PATH: upstream bpf-linker releases since 0.11.0 are built against the rustc
    // trunk LLVM pinned to 2026-08-12, so use the matching nightly. Distro
    // bpf-linkers linked against a stable llvm cannot lower trunk bitcode no
    // matter which nightly is pinned here (Arch's 0.11.0-1 vs llvm-libs 22 for
    // example); those builds need a bpf-linker matching their rustc's LLVM
    const EBPF_NIGHTLY: &str = "nightly-2026-08-12";
    aya_build::build_ebpf([ebpf_package], Toolchain::Custom(EBPF_NIGHTLY))
}
