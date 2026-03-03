use crate::errors::Result;
use crate::installer::SquirrelEvent;
use crate::version::BINARY_FILENAME;
use anyhow::Context;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn handle(event: SquirrelEvent) -> Result<bool> {
    match event {
        SquirrelEvent::Install | SquirrelEvent::Updated => {
            create_startup_and_start_menu_shortcuts()?;
            Ok(true)
        }
        SquirrelEvent::Uninstall => {
            remove_startup_and_start_menu_shortcuts()?;
            Ok(true)
        }
        SquirrelEvent::Obsolete => Ok(true),
        SquirrelEvent::Firstrun => Ok(false),
    }
}

fn create_startup_and_start_menu_shortcuts() -> Result<()> {
    let update_exe = locate_update_exe()?;
    let exe_name = binary_filename_with_extension();

    let variants = [
        vec![
            "--createShortcut".to_string(),
            exe_name.clone(),
            "--shortcut-locations=Startup,StartMenu".to_string(),
        ],
        vec![
            format!("--createShortcut={exe_name}"),
            "--shortcut-locations=Startup,StartMenu".to_string(),
        ],
        vec![
            "--createShortcut".to_string(),
            exe_name.clone(),
            "-l=Startup,StartMenu".to_string(),
        ],
        vec![
            format!("--createShortcut={exe_name}"),
            "-l=Startup,StartMenu".to_string(),
        ],
    ];

    run_update_variants(&update_exe, &variants, "create startup/start menu shortcuts")
}

fn remove_startup_and_start_menu_shortcuts() -> Result<()> {
    let update_exe = locate_update_exe()?;
    let exe_name = binary_filename_with_extension();

    let variants = [
        vec![
            "--removeShortcut".to_string(),
            exe_name.clone(),
            "--shortcut-locations=Startup,StartMenu".to_string(),
        ],
        vec![
            format!("--removeShortcut={exe_name}"),
            "--shortcut-locations=Startup,StartMenu".to_string(),
        ],
        vec![
            "--removeShortcut".to_string(),
            exe_name.clone(),
            "-l=Startup,StartMenu".to_string(),
        ],
        vec![
            format!("--removeShortcut={exe_name}"),
            "-l=Startup,StartMenu".to_string(),
        ],
    ];

    run_update_variants(&update_exe, &variants, "remove startup/start menu shortcuts")
}

fn locate_update_exe() -> Result<PathBuf> {
    let current_exe = std::env::current_exe().context("failed to resolve current executable")?;
    let app_dir = current_exe
        .parent()
        .with_context(|| format!("failed to resolve app directory for {}", current_exe.display()))?;
    let root_dir = app_dir.parent().with_context(|| {
        format!(
            "failed to resolve squirrel root directory from {}",
            app_dir.display()
        )
    })?;
    let update_exe = root_dir.join("Update.exe");
    if !update_exe.exists() {
        anyhow::bail!(
            "squirrel Update.exe was not found at {}; lifecycle event cannot continue",
            update_exe.display()
        );
    }
    Ok(update_exe)
}

fn run_update_variants(update_exe: &Path, variants: &[Vec<String>], action: &str) -> Result<()> {
    let mut failures = Vec::new();

    for args in variants {
        let output = Command::new(update_exe)
            .args(args)
            .output()
            .with_context(|| format!("failed to execute {}", update_exe.display()))?;

        if output.status.success() {
            return Ok(());
        }

        failures.push(format!(
            "args=[{}] status={} stderr={}",
            args.join(" "),
            output
                .status
                .code()
                .map(|value| value.to_string())
                .unwrap_or_else(|| "<signal>".to_string()),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    anyhow::bail!(
        "failed to {action} via {}: {}",
        update_exe.display(),
        failures.join(" | ")
    );
}

fn binary_filename_with_extension() -> String {
    format!("{BINARY_FILENAME}.exe")
}
