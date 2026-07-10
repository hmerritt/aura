use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallLayout {
    Cask,
    Formula,
    Unmanaged,
}

pub fn detect_install_layout(executable: &Path) -> InstallLayout {
    let normalized = executable.to_string_lossy().replace('\\', "/");
    if normalized.contains(".app/Contents/MacOS/") {
        InstallLayout::Cask
    } else if normalized.contains("/Cellar/aura/") || normalized.contains("/opt/homebrew/opt/aura/")
    {
        InstallLayout::Formula
    } else {
        InstallLayout::Unmanaged
    }
}

pub fn update_instruction(layout: InstallLayout) -> &'static str {
    match layout {
        InstallLayout::Cask => "brew upgrade --cask aura",
        InstallLayout::Formula => "brew upgrade aura",
        InstallLayout::Unmanaged => {
            "Install Aura with Homebrew, then use: brew upgrade --cask aura"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_cask_bundle() {
        assert_eq!(
            detect_install_layout(Path::new("/Applications/Aura.app/Contents/MacOS/aura")),
            InstallLayout::Cask
        );
    }

    #[test]
    fn detects_formula_prefixes() {
        assert_eq!(
            detect_install_layout(Path::new("/opt/homebrew/Cellar/aura/1.2.3/bin/aura")),
            InstallLayout::Formula
        );
        assert_eq!(
            detect_install_layout(Path::new("/opt/homebrew/opt/aura/bin/aura")),
            InstallLayout::Formula
        );
    }

    #[test]
    fn chooses_homebrew_instruction() {
        assert_eq!(
            update_instruction(InstallLayout::Cask),
            "brew upgrade --cask aura"
        );
        assert_eq!(
            update_instruction(InstallLayout::Formula),
            "brew upgrade aura"
        );
    }
}
