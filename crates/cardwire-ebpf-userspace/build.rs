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
    // NOTE: The build will fail if bpf-linker is not linked to llvm23
    // See <https://github.com/OpenGamingCollective/cardwire/issues/193>
    const EBPF_NIGHTLY: &str = "nightly-2026-08-12";
    aya_build::build_ebpf([ebpf_package], Toolchain::Custom(EBPF_NIGHTLY))
}
