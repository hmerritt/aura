use crate::errors::Result;
use anyhow::{bail, Context};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const RUSTGPU_TOOLCHAIN: &str = "nightly-2025-08-04";

#[derive(Debug, Clone)]
pub struct CompileOptions {
    pub shader_crate: PathBuf,
    pub output_spv: PathBuf,
}

pub fn compile_shader(options: &CompileOptions) -> Result<()> {
    let builder_manifest = builder_manifest_path()?;
    if !builder_manifest.exists() {
        bail!(
            "shader builder manifest not found at {}",
            builder_manifest.display()
        );
    }
    let builder_dir = builder_manifest
        .parent()
        .context("shader builder manifest has no parent directory")?;

    if !options.shader_crate.exists() || !options.shader_crate.is_dir() {
        bail!(
            "shader crate path does not exist or is not a directory: {}",
            options.shader_crate.display()
        );
    }
    if let Some(parent) = options.output_spv.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create shader cache dir {}", parent.display()))?;
    }

    let output = Command::new("cargo")
        .arg(format!("+{RUSTGPU_TOOLCHAIN}"))
        .arg("run")
        .arg("--release")
        .arg("--quiet")
        .arg("--manifest-path")
        .arg(&builder_manifest)
        .arg("--")
        .arg("--shader-crate")
        .arg(&options.shader_crate)
        .arg("--out")
        .arg(&options.output_spv)
        .current_dir(builder_dir)
        .output()
        .with_context(|| format!("failed to run shader builder at {}", builder_dir.display()))?;

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "shader compile failed\nstdout:\n{}\nstderr:\n{}\n\nrequired rust-gpu toolchain setup:\n  rustup toolchain install {}\n  rustup component add --toolchain {} rust-src rustc-dev llvm-tools",
            stdout.trim(),
            stderr.trim(),
            RUSTGPU_TOOLCHAIN,
            RUSTGPU_TOOLCHAIN
        );
    }

    if !options.output_spv.exists() {
        bail!(
            "shader builder succeeded but did not produce output: {}",
            options.output_spv.display()
        );
    }

    Ok(())
}

fn builder_manifest_path() -> Result<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    Ok(root
        .join("shaders")
        .join("shader_builder")
        .join("Cargo.toml"))
}
