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
    // The prebuilt bpf-linker v0.10.4 release bundles LLVM 22 and cannot link LLVM-23 bitcode
    // emitted by nightlies from 2026-08-05 onward (`ERROR llvm: Invalid record`). Pin the eBPF
    // build to the last compatible nightly. bump this once bpf-linker supports LLVM 23.
    const EBPF_NIGHTLY: &str = "nightly-2026-08-04";
    aya_build::build_ebpf([ebpf_package], Toolchain::Custom(EBPF_NIGHTLY))
}
