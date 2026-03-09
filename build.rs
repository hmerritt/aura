use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const TRAY_ICON_RESOURCE_ID: u16 = 101;
const NEXT_BACKGROUND_ICON_RESOURCE_ID: u16 = 203;
const REFRESH_ICON_RESOURCE_ID: u16 = 204;
const RELOAD_SETTINGS_ICON_RESOURCE_ID: u16 = 205;
const SETTINGS_ICON_RESOURCE_ID: u16 = 201;
const EXIT_ICON_RESOURCE_ID: u16 = 202;
const NEXT_BACKGROUND_ICON_FALLBACK_RESOURCE_ID: u16 = 303;
const REFRESH_ICON_FALLBACK_RESOURCE_ID: u16 = 304;
const RELOAD_SETTINGS_ICON_FALLBACK_RESOURCE_ID: u16 = 305;
const SETTINGS_ICON_FALLBACK_RESOURCE_ID: u16 = 301;
const EXIT_ICON_FALLBACK_RESOURCE_ID: u16 = 302;
// Keep this in sync with shaders/rust-toolchain.toml and CI workflows.
const RUSTGPU_TOOLCHAIN: &str = "nightly-2025-10-28";

fn main() {
    println!("cargo:rerun-if-changed=assets/tray.png");
    println!("cargo:rerun-if-changed=assets/menu-next-background.png");
    println!("cargo:rerun-if-changed=assets/menu-refresh.png");
    println!("cargo:rerun-if-changed=assets/rotate-reload.png");
    println!("cargo:rerun-if-changed=assets/menu-settings.png");
    println!("cargo:rerun-if-changed=assets/menu-exit.png");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-env-changed=AURA_VERSION_PRERELEASE");
    println!("cargo:rerun-if-env-changed=AURA_VERSION_METADATA");
    println!("cargo:rerun-if-env-changed=AURA_BUILD_DATE");

    let manifest_dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is required"));
    emit_version_metadata(&manifest_dir);

    let target = std::env::var("TARGET").unwrap_or_default();
    if !target.contains("windows") {
        return;
    }

    let out_dir = PathBuf::from(
        std::env::var("OUT_DIR").expect("OUT_DIR is required for resource generation"),
    );

    compile_precompiled_shaders(&manifest_dir, &out_dir);
    build_settings_ui_bundle(&manifest_dir, &out_dir);
    generate_windows_resources(&manifest_dir, &out_dir);
}

fn build_settings_ui_bundle(manifest_dir: &Path, out_dir: &Path) {
    let settings_ui_dir = manifest_dir.join("ui");
    if !settings_ui_dir.exists() {
        panic!(
            "missing settings UI project directory: {}",
            settings_ui_dir.display()
        );
    }

    emit_rerun_if_changed_recursive(&settings_ui_dir);

    let package_json = settings_ui_dir.join("package.json");
    let bun_lock = settings_ui_dir.join("bun.lock");
    if !package_json.exists() {
        panic!(
            "missing settings UI package manifest: {}",
            package_json.display()
        );
    }
    if !bun_lock.exists() {
        panic!(
            "missing settings UI lockfile: {}. Run `bun install` in ui first.",
            bun_lock.display()
        );
    }

    ensure_bun_available();

    let node_modules = settings_ui_dir.join("node_modules");
    let install_stamp = out_dir.join("ui-install.stamp");
    if settings_ui_install_is_stale(&package_json, &bun_lock, &node_modules, &install_stamp) {
        run_command_checked(
            Command::new("bun")
                .arg("install")
                .arg("--frozen-lockfile")
                .current_dir(&settings_ui_dir),
            "bun install --frozen-lockfile for ui",
        );
        fs::write(&install_stamp, b"ok").unwrap_or_else(|error| {
            panic!(
                "failed to update settings UI install stamp {}: {}",
                install_stamp.display(),
                error
            )
        });
    }

    run_command_checked(
        Command::new("bun")
            .arg("run")
            .arg("build")
            .current_dir(&settings_ui_dir),
        "bun run build for ui",
    );

    let dist_dir = settings_ui_dir.join("dist");
    if !dist_dir.exists() {
        panic!(
            "settings UI dist directory is missing after build: {}",
            dist_dir.display()
        );
    }

    let generated_assets = out_dir.join("settings_ui_assets.rs");
    write_settings_ui_asset_registry(&dist_dir, &generated_assets);
}

fn compile_precompiled_shaders(manifest_dir: &Path, out_dir: &Path) {
    let shaders_dir = manifest_dir.join("shaders");
    println!("cargo:rerun-if-changed={}", shaders_dir.display());
    let shader_builder_manifest = shaders_dir.join("shader_builder").join("Cargo.toml");
    if !shader_builder_manifest.exists() {
        panic!(
            "missing shader builder manifest: {}",
            shader_builder_manifest.display()
        );
    }

    emit_rerun_if_changed_recursive(&shaders_dir.join("rust-toolchain.toml"));
    emit_rerun_if_changed_recursive(&shaders_dir.join("shader_builder"));

    let shader_crates = discover_shader_crates(&shaders_dir);
    if shader_crates.is_empty() {
        panic!(
            "no shader crates found in {}; expected at least one crate besides shader_builder",
            shaders_dir.display()
        );
    }

    let compiled_dir = out_dir.join("precompiled_shaders");
    fs::create_dir_all(&compiled_dir).unwrap_or_else(|error| {
        panic!(
            "failed to create precompiled shader output directory {}: {}",
            compiled_dir.display(),
            error
        )
    });

    let mut compiled = Vec::with_capacity(shader_crates.len());
    for (shader_name, shader_crate_dir) in shader_crates {
        emit_rerun_if_changed_recursive(&shader_crate_dir);

        let output_spv = compiled_dir.join(format!("{shader_name}.spv"));
        let output = Command::new("rustup")
            .arg("run")
            .arg(RUSTGPU_TOOLCHAIN)
            .arg("cargo")
            .arg("run")
            .arg("--release")
            .arg("--quiet")
            .arg("--manifest-path")
            .arg(&shader_builder_manifest)
            .arg("--")
            .arg("--shader-crate")
            .arg(&shader_crate_dir)
            .arg("--out")
            .arg(&output_spv)
            .env_remove("RUSTC")
            .env_remove("RUSTDOC")
            .env_remove("RUSTUP_TOOLCHAIN")
            .current_dir(manifest_dir)
            .output()
            .unwrap_or_else(|error| {
                panic!(
                    "failed to execute shader build for {}: {}",
                    shader_name, error
                )
            });

        if !output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            panic!(
                "shader build failed for {}\nstdout:\n{}\nstderr:\n{}",
                shader_name,
                stdout.trim(),
                stderr.trim()
            );
        }

        if !output_spv.exists() {
            panic!(
                "shader build succeeded but output is missing for {}: {}",
                shader_name,
                output_spv.display()
            );
        }

        compiled.push((shader_name, output_spv));
    }

    write_shader_registry(out_dir, &compiled);
}

fn discover_shader_crates(shaders_dir: &Path) -> Vec<(String, PathBuf)> {
    let mut shader_crates = Vec::new();

    let entries = fs::read_dir(shaders_dir)
        .unwrap_or_else(|error| panic!("failed to read {}: {}", shaders_dir.display(), error));
    for entry in entries {
        let entry = entry.unwrap_or_else(|error| {
            panic!(
                "failed to read an entry from {}: {}",
                shaders_dir.display(),
                error
            )
        });
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let shader_name = entry.file_name().to_string_lossy().to_string();
        if shader_name == "shader_builder" {
            continue;
        }
        if !path.join("Cargo.toml").exists() {
            continue;
        }

        shader_crates.push((shader_name, path));
    }

    shader_crates.sort_by(|a, b| a.0.cmp(&b.0));
    shader_crates
}

fn write_shader_registry(out_dir: &Path, compiled: &[(String, PathBuf)]) {
    let mut source = String::from("&[\n");
    for (shader_name, shader_path) in compiled {
        let shader_name_literal = shader_name.replace('"', "\\\"");
        let shader_path_literal = shader_path
            .to_string_lossy()
            .replace('\\', "/")
            .replace('"', "\\\"");
        source.push_str(&format!(
            "    (\"{}\", include_bytes!(\"{}\") as &[u8]),\n",
            shader_name_literal, shader_path_literal
        ));
    }
    source.push_str("]\n");

    let registry_path = out_dir.join("precompiled_shaders.rs");
    fs::write(&registry_path, source).unwrap_or_else(|error| {
        panic!(
            "failed to write shader registry {}: {}",
            registry_path.display(),
            error
        )
    });
}

fn emit_rerun_if_changed_recursive(path: &Path) {
    if !path.exists() {
        return;
    }

    if path.is_file() {
        println!("cargo:rerun-if-changed={}", path.display());
        return;
    }

    if path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| matches!(name, "target" | "dist" | "node_modules"))
        .unwrap_or(false)
    {
        return;
    }

    let entries = fs::read_dir(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {}", path.display(), error));
    for entry in entries {
        let entry = entry.unwrap_or_else(|error| {
            panic!("failed to read an entry from {}: {}", path.display(), error)
        });
        emit_rerun_if_changed_recursive(&entry.path());
    }
}

fn ensure_bun_available() {
    let output = Command::new("bun").arg("--version").output();
    match output {
        Ok(output) if output.status.success() => {}
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            panic!(
                "bun is required to build the settings UI bundle.\nstdout:\n{}\nstderr:\n{}",
                stdout.trim(),
                stderr.trim()
            );
        }
        Err(error) => {
            panic!(
                "bun is required to build the settings UI bundle but was not found on PATH: {}",
                error
            );
        }
    }
}

fn settings_ui_install_is_stale(
    package_json: &Path,
    bun_lock: &Path,
    node_modules: &Path,
    install_stamp: &Path,
) -> bool {
    if !node_modules.exists() || !install_stamp.exists() {
        return true;
    }

    let stamp_modified = file_modified_time(install_stamp);
    file_modified_time(package_json) > stamp_modified
        || file_modified_time(bun_lock) > stamp_modified
}

fn file_modified_time(path: &Path) -> SystemTime {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .unwrap_or(UNIX_EPOCH)
}

fn run_command_checked(command: &mut Command, description: &str) {
    let output = command.output().unwrap_or_else(|error| {
        panic!("failed to execute {}: {}", description, error);
    });

    if output.status.success() {
        return;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    panic!(
        "{} failed with status {}\nstdout:\n{}\nstderr:\n{}",
        description,
        output.status,
        stdout.trim(),
        stderr.trim()
    );
}

fn write_settings_ui_asset_registry(dist_dir: &Path, generated_assets: &Path) {
    let mut assets = Vec::new();
    collect_files_recursive(dist_dir, dist_dir, &mut assets);
    if assets.is_empty() {
        panic!(
            "settings UI build produced no assets in {}",
            dist_dir.display()
        );
    }

    assets.sort_by(|a, b| a.0.cmp(&b.0));

    let mut source = String::from("&[\n");
    for (relative_path, asset_path) in assets {
        let escaped_relative = relative_path.replace('\\', "/").replace('"', "\\\"");
        let escaped_asset_path = asset_path
            .to_string_lossy()
            .replace('\\', "/")
            .replace('"', "\\\"");
        let mime_type = settings_ui_mime_type(&relative_path);
        source.push_str(&format!(
            "    (\"{}\", \"{}\", include_bytes!(\"{}\") as &[u8]),\n",
            escaped_relative, mime_type, escaped_asset_path
        ));
    }
    source.push_str("]\n");

    fs::write(generated_assets, source).unwrap_or_else(|error| {
        panic!(
            "failed to write generated settings UI asset registry {}: {}",
            generated_assets.display(),
            error
        )
    });
}

fn collect_files_recursive(root: &Path, current: &Path, assets: &mut Vec<(String, PathBuf)>) {
    let entries = fs::read_dir(current)
        .unwrap_or_else(|error| panic!("failed to read {}: {}", current.display(), error));
    for entry in entries {
        let entry = entry.unwrap_or_else(|error| {
            panic!(
                "failed to read an entry from {}: {}",
                current.display(),
                error
            )
        });
        let path = entry.path();
        if path.is_dir() {
            collect_files_recursive(root, &path, assets);
            continue;
        }

        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        assets.push((relative, path));
    }
}

fn settings_ui_mime_type(path: &str) -> &'static str {
    if path.ends_with(".html") {
        "text/html; charset=utf-8"
    } else if path.ends_with(".js") || path.ends_with(".mjs") {
        "text/javascript; charset=utf-8"
    } else if path.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if path.ends_with(".json") || path.ends_with(".map") {
        "application/json; charset=utf-8"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".jpg") || path.ends_with(".jpeg") {
        "image/jpeg"
    } else if path.ends_with(".gif") {
        "image/gif"
    } else if path.ends_with(".webp") {
        "image/webp"
    } else if path.ends_with(".ico") {
        "image/x-icon"
    } else if path.ends_with(".woff") {
        "font/woff"
    } else if path.ends_with(".woff2") {
        "font/woff2"
    } else if path.ends_with(".ttf") {
        "font/ttf"
    } else if path.ends_with(".otf") {
        "font/otf"
    } else if path.ends_with(".txt") {
        "text/plain; charset=utf-8"
    } else {
        "application/octet-stream"
    }
}

fn generate_windows_resources(manifest_dir: &Path, out_dir: &Path) {
    let source_png = manifest_dir.join("assets").join("tray.png");
    if !source_png.exists() {
        panic!("missing source tray image: {}", source_png.display());
    }
    let next_background_source_png = manifest_dir.join("assets").join("menu-next-background.png");
    if !next_background_source_png.exists() {
        panic!(
            "missing source menu next background image: {}",
            next_background_source_png.display()
        );
    }
    let refresh_source_png = manifest_dir.join("assets").join("menu-refresh.png");
    if !refresh_source_png.exists() {
        panic!(
            "missing source menu refresh image: {}",
            refresh_source_png.display()
        );
    }
    let reload_settings_source_png = manifest_dir.join("assets").join("rotate-reload.png");
    if !reload_settings_source_png.exists() {
        panic!(
            "missing source menu reload settings image: {}",
            reload_settings_source_png.display()
        );
    }
    let settings_source_png = manifest_dir.join("assets").join("menu-settings.png");
    if !settings_source_png.exists() {
        panic!(
            "missing source menu settings image: {}",
            settings_source_png.display()
        );
    }
    let exit_source_png = manifest_dir.join("assets").join("menu-exit.png");
    if !exit_source_png.exists() {
        panic!(
            "missing source menu exit image: {}",
            exit_source_png.display()
        );
    }

    let generated_ico = out_dir.join("tray.ico");
    generate_multi_size_ico(&source_png, &generated_ico);
    let generated_next_background_bmp = out_dir.join("menu-next-background.bmp");
    generate_menu_bitmap(&next_background_source_png, &generated_next_background_bmp);
    let generated_refresh_bmp = out_dir.join("menu-refresh.bmp");
    generate_menu_bitmap(&refresh_source_png, &generated_refresh_bmp);
    let generated_reload_settings_bmp = out_dir.join("menu-rotate-reload.bmp");
    generate_menu_bitmap(&reload_settings_source_png, &generated_reload_settings_bmp);
    let generated_settings_bmp = out_dir.join("menu-settings.bmp");
    generate_menu_bitmap(&settings_source_png, &generated_settings_bmp);
    let generated_exit_bmp = out_dir.join("menu-exit.bmp");
    generate_menu_bitmap(&exit_source_png, &generated_exit_bmp);
    let generated_next_background_ico = out_dir.join("menu-next-background.ico");
    generate_menu_icon(&next_background_source_png, &generated_next_background_ico);
    let generated_refresh_ico = out_dir.join("menu-refresh.ico");
    generate_menu_icon(&refresh_source_png, &generated_refresh_ico);
    let generated_reload_settings_ico = out_dir.join("menu-rotate-reload.ico");
    generate_menu_icon(&reload_settings_source_png, &generated_reload_settings_ico);
    let generated_settings_ico = out_dir.join("menu-settings.ico");
    generate_menu_icon(&settings_source_png, &generated_settings_ico);
    let generated_exit_ico = out_dir.join("menu-exit.ico");
    generate_menu_icon(&exit_source_png, &generated_exit_ico);

    let generated_rc = out_dir.join("aura-auto.rc");
    let ico_path_for_rc = generated_ico.to_string_lossy().replace('\\', "/");
    let next_background_bmp_path_for_rc = generated_next_background_bmp
        .to_string_lossy()
        .replace('\\', "/");
    let refresh_bmp_path_for_rc = generated_refresh_bmp.to_string_lossy().replace('\\', "/");
    let reload_settings_bmp_path_for_rc = generated_reload_settings_bmp
        .to_string_lossy()
        .replace('\\', "/");
    let settings_bmp_path_for_rc = generated_settings_bmp.to_string_lossy().replace('\\', "/");
    let exit_bmp_path_for_rc = generated_exit_bmp.to_string_lossy().replace('\\', "/");
    let next_background_ico_path_for_rc = generated_next_background_ico
        .to_string_lossy()
        .replace('\\', "/");
    let refresh_ico_path_for_rc = generated_refresh_ico.to_string_lossy().replace('\\', "/");
    let reload_settings_ico_path_for_rc = generated_reload_settings_ico
        .to_string_lossy()
        .replace('\\', "/");
    let settings_ico_path_for_rc = generated_settings_ico.to_string_lossy().replace('\\', "/");
    let exit_ico_path_for_rc = generated_exit_ico.to_string_lossy().replace('\\', "/");
    let rc_payload = format!(
        "{} ICON \"{}\"\n{} BITMAP \"{}\"\n{} BITMAP \"{}\"\n{} BITMAP \"{}\"\n{} BITMAP \"{}\"\n{} BITMAP \"{}\"\n{} ICON \"{}\"\n{} ICON \"{}\"\n{} ICON \"{}\"\n{} ICON \"{}\"\n{} ICON \"{}\"\n",
        TRAY_ICON_RESOURCE_ID,
        ico_path_for_rc,
        NEXT_BACKGROUND_ICON_RESOURCE_ID,
        next_background_bmp_path_for_rc,
        REFRESH_ICON_RESOURCE_ID,
        refresh_bmp_path_for_rc,
        RELOAD_SETTINGS_ICON_RESOURCE_ID,
        reload_settings_bmp_path_for_rc,
        SETTINGS_ICON_RESOURCE_ID,
        settings_bmp_path_for_rc,
        EXIT_ICON_RESOURCE_ID,
        exit_bmp_path_for_rc,
        NEXT_BACKGROUND_ICON_FALLBACK_RESOURCE_ID,
        next_background_ico_path_for_rc,
        REFRESH_ICON_FALLBACK_RESOURCE_ID,
        refresh_ico_path_for_rc,
        RELOAD_SETTINGS_ICON_FALLBACK_RESOURCE_ID,
        reload_settings_ico_path_for_rc,
        SETTINGS_ICON_FALLBACK_RESOURCE_ID,
        settings_ico_path_for_rc,
        EXIT_ICON_FALLBACK_RESOURCE_ID,
        exit_ico_path_for_rc
    );
    fs::write(&generated_rc, rc_payload).expect("failed to write generated rc file");

    let generated_rc_str = generated_rc
        .to_str()
        .expect("generated rc path must be valid UTF-8");
    let _ = embed_resource::compile(generated_rc_str, embed_resource::NONE);
}

fn generate_multi_size_ico(source_png: &Path, output_ico: &Path) {
    let source = image::open(source_png)
        .unwrap_or_else(|e| panic!("failed to load {}: {}", source_png.display(), e))
        .to_rgba8();

    let sizes = [16u32, 20, 24, 32, 48, 64, 256];

    let mut icon_dir = ico::IconDir::new(ico::ResourceType::Icon);
    for size in sizes {
        let resized =
            image::imageops::resize(&source, size, size, image::imageops::FilterType::Lanczos3);
        let icon_image = ico::IconImage::from_rgba_data(size, size, resized.into_raw());
        let entry = ico::IconDirEntry::encode(&icon_image)
            .unwrap_or_else(|e| panic!("failed to encode {}x{} icon entry: {}", size, size, e));
        icon_dir.add_entry(entry);
    }

    let mut file = fs::File::create(output_ico)
        .unwrap_or_else(|e| panic!("failed to create {}: {}", output_ico.display(), e));
    icon_dir
        .write(&mut file)
        .unwrap_or_else(|e| panic!("failed to write {}: {}", output_ico.display(), e));
}

fn generate_menu_bitmap(source_png: &Path, output_bmp: &Path) {
    let source = image::open(source_png)
        .unwrap_or_else(|e| panic!("failed to load {}: {}", source_png.display(), e))
        .to_rgba8();
    let resized = image::imageops::resize(&source, 16, 16, image::imageops::FilterType::Lanczos3);
    image::DynamicImage::ImageRgba8(resized)
        .save_with_format(output_bmp, image::ImageFormat::Bmp)
        .unwrap_or_else(|e| panic!("failed to write {}: {}", output_bmp.display(), e));
}

fn generate_menu_icon(source_png: &Path, output_ico: &Path) {
    let source = image::open(source_png)
        .unwrap_or_else(|e| panic!("failed to load {}: {}", source_png.display(), e))
        .to_rgba8();
    let resized = image::imageops::resize(&source, 16, 16, image::imageops::FilterType::Lanczos3);

    let mut icon_dir = ico::IconDir::new(ico::ResourceType::Icon);
    let icon_image = ico::IconImage::from_rgba_data(16, 16, resized.into_raw());
    let entry = ico::IconDirEntry::encode(&icon_image)
        .unwrap_or_else(|e| panic!("failed to encode 16x16 icon entry: {}", e));
    icon_dir.add_entry(entry);

    let mut file = fs::File::create(output_ico)
        .unwrap_or_else(|e| panic!("failed to create {}: {}", output_ico.display(), e));
    icon_dir
        .write(&mut file)
        .unwrap_or_else(|e| panic!("failed to write {}: {}", output_ico.display(), e));
}

fn emit_version_metadata(manifest_dir: &Path) {
    let git_commit = run_git(&["rev-parse", "--short", "HEAD"]).unwrap_or_default();
    let git_branch = run_git(&["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_default();

    let build_date = std::env::var("AURA_BUILD_DATE")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(current_unix_timestamp);

    let version_prerelease = std::env::var("AURA_VERSION_PRERELEASE").unwrap_or_default();
    let version_metadata = std::env::var("AURA_VERSION_METADATA").unwrap_or_default();
    let publisher = read_publisher_from_nuspec(manifest_dir);

    println!("cargo:rustc-env=AURA_GIT_COMMIT={git_commit}");
    println!("cargo:rustc-env=AURA_GIT_BRANCH={git_branch}");
    println!("cargo:rustc-env=AURA_BUILD_DATE={build_date}");
    println!("cargo:rustc-env=AURA_VERSION_PRERELEASE={version_prerelease}");
    println!("cargo:rustc-env=AURA_VERSION_METADATA={version_metadata}");
    println!("cargo:rustc-env=AURA_PUBLISHER={publisher}");
}

fn read_publisher_from_nuspec(manifest_dir: &Path) -> String {
    let nuspec_path = manifest_dir
        .join("packaging")
        .join("windows")
        .join("squirrel")
        .join("aura.nuspec");
    println!("cargo:rerun-if-changed={}", nuspec_path.display());

    let nuspec_contents = fs::read_to_string(&nuspec_path).unwrap_or_else(|error| {
        panic!(
            "failed to read nuspec metadata {}: {}",
            nuspec_path.display(),
            error
        )
    });

    extract_xml_element(&nuspec_contents, "authors")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            panic!(
                "missing non-empty <authors> value in {}",
                nuspec_path.display()
            )
        })
        .to_string()
}

fn extract_xml_element<'a>(text: &'a str, tag: &str) -> Option<&'a str> {
    let start_tag = format!("<{tag}>");
    let end_tag = format!("</{tag}>");

    let start_index = text.find(&start_tag)? + start_tag.len();
    let end_index = text[start_index..].find(&end_tag)? + start_index;
    Some(&text[start_index..end_index])
}

fn run_git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8(output.stdout).ok()?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn current_unix_timestamp() -> String {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs().to_string(),
        Err(_) => String::new(),
    }
}
