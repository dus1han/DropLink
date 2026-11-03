//! Operating-system integrations that differ per platform.

/// Windows: a shortcut in the user's `SendTo` folder adds "DropLink" to
/// Explorer's *Send to* context menu. Explorer launches DropLink with the
/// selected paths as arguments; the single-instance plugin forwards them to
/// the running window.
#[cfg(windows)]
mod imp {
    use std::os::windows::process::CommandExt;
    use std::path::PathBuf;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    fn shortcut_path() -> Option<PathBuf> {
        std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join(r"Microsoft\Windows\SendTo\DropLink.lnk"))
    }

    pub fn send_to_installed() -> bool {
        shortcut_path().map(|p| p.exists()).unwrap_or(false)
    }

    pub fn set_send_to(enabled: bool) -> Result<(), String> {
        let link = shortcut_path().ok_or("APPDATA is not set")?;
        if !enabled {
            if link.exists() {
                std::fs::remove_file(&link).map_err(|e| e.to_string())?;
            }
            return Ok(());
        }
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        // Paths are passed through environment variables so no quoting or
        // escaping of user-controlled strings happens inside the script.
        let script = "$s = (New-Object -ComObject WScript.Shell).CreateShortcut($env:DL_LINK); \
                      $s.TargetPath = $env:DL_EXE; $s.IconLocation = $env:DL_EXE + ',0'; \
                      $s.Description = 'Send with DropLink'; $s.Save()";
        let status = Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script])
            .env("DL_LINK", &link)
            .env("DL_EXE", &exe)
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .map_err(|e| e.to_string())?;
        if status.success() && link.exists() {
            Ok(())
        } else {
            Err("could not create the Send to shortcut".into())
        }
    }
}

/// macOS and Linux have no equivalent that does not require a separate
/// signed app extension (macOS Share/Services). Files can be dropped on the
/// window or the Dock icon instead; see docs/PLATFORM_NOTES.md.
#[cfg(not(windows))]
mod imp {
    pub fn send_to_installed() -> bool {
        false
    }
    pub fn set_send_to(_enabled: bool) -> Result<(), String> {
        Err("the Send to menu is only available on Windows".into())
    }
}

pub use imp::{send_to_installed, set_send_to};

/// Extracts existing file paths from a command line.
pub fn paths_from_args<I: IntoIterator<Item = String>>(args: I) -> Vec<String> {
    args.into_iter()
        .skip(1)
        .filter(|a| !a.starts_with('-'))
        .filter(|a| std::path::Path::new(a).exists())
        .collect()
}
