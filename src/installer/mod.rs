use crate::errors::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SquirrelEvent {
    Install,
    Updated,
    Uninstall,
    Obsolete,
    Firstrun,
}

impl SquirrelEvent {
    pub fn from_flag(flag: &str) -> Option<Self> {
        match flag {
            "--squirrel-install" => Some(Self::Install),
            "--squirrel-updated" => Some(Self::Updated),
            "--squirrel-uninstall" => Some(Self::Uninstall),
            "--squirrel-obsolete" => Some(Self::Obsolete),
            "--squirrel-firstrun" => Some(Self::Firstrun),
            _ => None,
        }
    }
}

#[cfg(windows)]
mod windows_squirrel;

#[cfg(windows)]
pub fn handle_squirrel_event(event: Option<SquirrelEvent>) -> Result<bool> {
    let Some(event) = event else {
        return Ok(false);
    };
    windows_squirrel::handle(event)
}

#[cfg(not(windows))]
pub fn handle_squirrel_event(event: Option<SquirrelEvent>) -> Result<bool> {
    let _ = event;
    Ok(false)
}
