# Shader golden images

Aura's Windows CI renders every portable shader through a windowless D3D12 wgpu pipeline and compares it with images produced by the pre-migration Rust-GPU shaders. The committed baseline is fixed to commit `fb226d6`; Rust-GPU is not part of Aura's normal build or CI toolchain.

The suite captures two 128×128 `Rgba8Unorm` frames per shader:

- `initial`: time 0, frame 0, mouse disabled at `(0, 0)`.
- `animated`: time 3.25 seconds, frame 195, mouse enabled at `(96, 40)`.

Every frame must have luma SSIM of at least 0.985, RGB RMSE no greater than 0.030, RGB mean absolute error no greater than 0.018, no more than 2% of pixels with a channel error above 0.12, and an exact alpha channel. Failures write the current frame, baseline frame, and an amplified difference image to `target/shader-golden-failures`.

## Running the comparison on Windows

Run the ignored GPU suite explicitly on Windows:

```powershell
$env:WGPU_BACKEND = 'dx12'
cargo test --locked renderer::golden_tests::portable_shaders_match_legacy_goldens -- --ignored --exact --test-threads=1
```

The test requests the Microsoft D3D12 fallback adapter first for stable results, then tries a hardware D3D12 adapter. It fails if neither is available.

## Running the comparison on macOS

The Metal comparison is optional and runs only when invoked manually. Golden-image output can vary between Metal devices, including virtualized CI hardware, so it is not a release or CI gate.

```bash
WGPU_BACKEND=metal cargo test --locked --target aarch64-apple-darwin renderer::golden_tests::portable_shaders_match_legacy_goldens -- --ignored --exact --test-threads=1
```

## Regenerating the baseline

Baseline regeneration is a maintainer operation. Build the six legacy SPIR-V modules from the pinned source and toolchain in a detached worktree:

```powershell
$legacy = Join-Path $env:TEMP 'aura-golden-fb226d6'
$output = Join-Path $env:TEMP 'aura-legacy-spv-fb226d6'
$shaders = @('dither_asci_1', 'dither_asci_2', 'dither_warp', 'gradient_glossy', 'limestone_cave', 'silk')

git worktree add --detach $legacy fb226d6
New-Item -ItemType Directory -Force -Path $output | Out-Null
foreach ($shader in $shaders) {
    cargo +nightly-2026-05-22 run --release `
        --manifest-path (Join-Path $legacy 'shaders\shader_builder\Cargo.toml') -- `
        --shader-crate (Join-Path $legacy "shaders\$shader") `
        --out (Join-Path $output "$shader.spv")
}
```

Then run the guarded regeneration test:

```powershell
$env:AURA_UPDATE_SHADER_GOLDENS = '1'
$env:AURA_LEGACY_SHADER_DIR = $output
$env:WGPU_BACKEND = 'dx12'
cargo test --locked renderer::golden_tests::regenerate_legacy_shader_goldens -- --ignored --exact --test-threads=1
```

The test checks that the existing manifest names `fb226d6`, renders legacy and portable shaders on the same adapter, and refuses to update any baseline unless all 12 comparisons pass the fixed thresholds. It then writes the PNGs and their BLAKE3 hashes to `tests/golden/shaders`.
