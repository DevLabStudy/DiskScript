use std::process::Command;

use crate::i18n::Lang;
use crate::tr;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum SysError {
    Unsupported,
    PowerShellStart(String),
    GetDisk(String),
    NoDisks,
    TempFile(String),
    DiskpartStart(String),
    DiskpartFailed(String),
    Elevation(String),
}

impl SysError {
    pub fn text(&self, lang: Lang) -> String {
        match self {
            SysError::Unsupported => tr!(
                lang,
                "ta funkcja działa tylko na Windows (diskpart)",
                "this feature only works on Windows (diskpart)"
            ),
            SysError::PowerShellStart(e) => tr!(
                lang,
                "nie można uruchomić PowerShell: {}",
                "cannot start PowerShell: {}",
                e
            ),
            SysError::GetDisk(e) => tr!(
                lang,
                "Get-Disk zwrócił błąd (uruchom program jako administrator): {}",
                "Get-Disk failed (run the program as administrator): {}",
                e
            ),
            SysError::NoDisks => tr!(
                lang,
                "nie znaleziono dysków (uruchom program jako administrator)",
                "no disks found (run the program as administrator)"
            ),
            SysError::TempFile(e) => tr!(
                lang,
                "nie można zapisać pliku tymczasowego: {}",
                "cannot write the temporary file: {}",
                e
            ),
            SysError::DiskpartStart(e) => tr!(
                lang,
                "nie można uruchomić diskpart (uruchom jako administrator): {}",
                "cannot start diskpart (run as administrator): {}",
                e
            ),
            SysError::DiskpartFailed(out) => tr!(
                lang,
                "diskpart zakończył się błędem:\n{}",
                "diskpart finished with an error:\n{}",
                out
            ),
            SysError::Elevation(e) => tr!(
                lang,
                "nie udało się uzyskać uprawnień administratora: {}",
                "could not obtain administrator privileges: {}",
                e
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DiskInfo {
    pub number: u32,
    pub name: String,
    pub size_bytes: u64,
    pub bus: String,
    pub is_system: bool,
}

impl DiskInfo {
    pub fn size_mb(&self) -> u64 {
        self.size_bytes / 1024 / 1024
    }

    pub fn describe(&self, lang: Lang) -> String {
        let gb = self.size_bytes as f64 / 1024.0 / 1024.0 / 1024.0;
        tr!(
            lang,
            "Dysk {}  •  {}  •  {:.1} GB  •  {}",
            "Disk {}  •  {}  •  {:.1} GB  •  {}",
            self.number,
            self.name,
            gb,
            self.bus
        )
    }
}

/// Sprawdza, czy dana litera jest już zajęta w systemie.
pub fn letter_in_use(c: char) -> bool {
    std::path::Path::new(&format!("{}:\\", c)).exists()
}

#[cfg(windows)]
fn hidden(cmd: &mut Command) -> &mut Command {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(0x0800_0000) // CREATE_NO_WINDOW
}

#[cfg(windows)]
pub fn list_disks() -> Result<Vec<DiskInfo>, SysError> {
    let ps = r#"Get-Disk | ForEach-Object { "$($_.Number)|$($_.FriendlyName)|$($_.Size)|$($_.BusType)|$($_.IsSystem)|$($_.IsBoot)" }"#;

    let mut cmd = Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", ps]);
    let out = hidden(&mut cmd)
        .output()
        .map_err(|e| SysError::PowerShellStart(e.to_string()))?;

    if !out.status.success() {
        return Err(SysError::GetDisk(
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ));
    }

    let text = String::from_utf8_lossy(&out.stdout);
    let mut disks = Vec::new();
    for line in text.lines() {
        let parts: Vec<&str> = line.trim().split('|').collect();
        if parts.len() != 6 {
            continue;
        }
        let (Ok(number), Ok(size)) = (parts[0].parse::<u32>(), parts[2].parse::<u64>()) else {
            continue;
        };
        disks.push(DiskInfo {
            number,
            name: parts[1].trim().to_string(),
            size_bytes: size,
            bus: parts[3].trim().to_string(),
            is_system: parts[4].eq_ignore_ascii_case("true")
                || parts[5].eq_ignore_ascii_case("true"),
        });
    }
    disks.sort_by_key(|d| d.number);

    if disks.is_empty() {
        Err(SysError::NoDisks)
    } else {
        Ok(disks)
    }
}

#[cfg(not(windows))]
pub fn list_disks() -> Result<Vec<DiskInfo>, SysError> {
    Err(SysError::Unsupported)
}

#[cfg(windows)]
pub fn run_diskpart(script_text: &str) -> Result<String, SysError> {
    let id = std::process::id();
    let temp = std::env::temp_dir();
    let path = temp.join(format!("diskscript_run_{}.txt", id));
    let output_path = temp.join(format!("diskscript_output_{}.txt", id));
    std::fs::write(&path, script_text).map_err(|e| SysError::TempFile(e.to_string()))?;

    let result = elevate_and_wait(&path, &output_path);
    let _ = std::fs::remove_file(&path);
    let output = std::fs::read_to_string(&output_path).unwrap_or_default();
    let _ = std::fs::remove_file(&output_path);

    match result {
        Ok(0) if !output.is_empty() => Ok(output),
        Ok(0) => Ok("diskpart zakończył działanie.".to_string()),
        Ok(_) => Err(SysError::DiskpartFailed(output)),
        Err(e) => Err(SysError::Elevation(e)),
    }
}

#[cfg(windows)]
pub fn run_elevated_child(script_path: &str, output_path: &str) -> i32 {
    let result = (|| {
        let mut cmd = Command::new("diskpart");
        cmd.arg("/s").arg(script_path);
        let out = hidden(&mut cmd)
            .output()
            .map_err(|e| SysError::DiskpartStart(e.to_string()))?;
        let mut text = String::from_utf8_lossy(&out.stdout).to_string();
        if !out.stderr.is_empty() {
            text.push_str(&String::from_utf8_lossy(&out.stderr));
        }
        std::fs::write(output_path, text).map_err(|e| SysError::TempFile(e.to_string()))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(SysError::DiskpartFailed(String::new()))
        }
    })();

    match result {
        Ok(()) => 0,
        Err(error) => {
            let _ = std::fs::write(output_path, error.text(Lang::Pl));
            1
        }
    }
}

#[cfg(windows)]
fn elevate_and_wait(
    script_path: &std::path::Path,
    output_path: &std::path::Path,
) -> Result<u32, String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, HWND};
    use windows::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject, INFINITE};
    use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};

    fn wide(value: &std::ffi::OsStr) -> Vec<u16> {
        value.encode_wide().chain(std::iter::once(0)).collect()
    }

    let executable_path = std::env::current_exe().map_err(|e| e.to_string())?;
    let executable = wide(executable_path.as_os_str());
    let params = format!(
        "--elevated-run \"{}\" \"{}\"",
        script_path.display(),
        output_path.display()
    );
    let params = wide(std::ffi::OsStr::new(&params));
    let verb = wide(std::ffi::OsStr::new("runas"));
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        hwnd: HWND::default(),
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(executable.as_ptr()),
        lpParameters: PCWSTR(params.as_ptr()),
        nShow: 0,
        ..Default::default()
    };

    unsafe {
        ShellExecuteExW(&mut info).map_err(|e| e.to_string())?;
        if info.hProcess.is_invalid() {
            return Err("Windows nie zwrócił uchwytu procesu".to_string());
        }
        let process = info.hProcess;
        WaitForSingleObject(process, INFINITE);
        let mut code = 1;
        GetExitCodeProcess(process, &mut code).map_err(|e| e.to_string())?;
        CloseHandle(process).map_err(|e| e.to_string())?;
        Ok(code)
    }
}

#[cfg(not(windows))]
pub fn run_diskpart(_script_text: &str) -> Result<String, SysError> {
    Err(SysError::Unsupported)
}
