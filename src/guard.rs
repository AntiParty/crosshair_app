//! "Is the game closed?" checks. Layered on purpose: a process scan, then an exclusive-open test of the
//! settings file. Both are repeated immediately before anything is written.

use std::fs::OpenOptions;
use std::os::windows::fs::OpenOptionsExt;
use std::path::Path;

use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};

/// True for THE FINALS' own executables: Discovery.exe (launcher stub), Discovery-d.exe, Discovery-e.exe, ...
pub fn is_game_process(exe: &str) -> bool {
    let n = exe.to_ascii_lowercase();
    n.ends_with(".exe") && (n == "discovery.exe" || n.starts_with("discovery-"))
}

/// Name of a running game process, or None if the game is closed. One cheap process snapshot.
pub fn running_game() -> Option<String> {
    // SAFETY: plain ToolHelp snapshot calls; the handle is closed before returning.
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE {
            return None;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut found = None;
        let mut ok = Process32FirstW(snap, &mut entry);
        while ok != 0 {
            let len = entry
                .szExeFile
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..len]);
            if is_game_process(&name) {
                found = Some(name);
                break;
            }
            ok = Process32NextW(snap, &mut entry);
        }
        CloseHandle(snap);
        found
    }
}

/// Opens the file exclusively (no sharing). Fails if anything else has it open.
pub fn file_is_free(path: &Path) -> bool {
    OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(0)
        .open(path)
        .is_ok()
}

#[derive(Debug, PartialEq)]
pub enum Block {
    GameRunning(String),
    FileBusy,
}

impl Block {
    pub fn message(&self) -> String {
        match self {
            Block::GameRunning(n) => format!("THE FINALS is running ({n}). Close it first."),
            Block::FileBusy => "The settings file is in use by another program.".into(),
        }
    }
}

/// Everything that must be true before a write. Call again right before writing.
pub fn check_clear(path: &Path) -> Result<(), Block> {
    if let Some(n) = running_game() {
        return Err(Block::GameRunning(n));
    }
    if path.exists() && !file_is_free(path) {
        return Err(Block::FileBusy);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_game_processes_only() {
        assert!(is_game_process("Discovery.exe"));
        assert!(is_game_process("discovery-d.exe"));
        assert!(is_game_process("Discovery-e.exe"));
        assert!(!is_game_process("DiscoveryPlus.exe"));
        assert!(!is_game_process("finals_crosshair.exe"));
        assert!(!is_game_process("Discord.exe"));
    }

    #[test]
    fn a_free_file_passes_and_a_held_file_is_busy() {
        let p = std::env::temp_dir().join("finals_guard_test.bin");
        std::fs::write(&p, b"x").unwrap();
        assert!(file_is_free(&p));
        let held = OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(0)
            .open(&p)
            .unwrap();
        assert!(!file_is_free(&p));
        drop(held);
        assert!(file_is_free(&p));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn process_scan_runs() {
        // The game isn't expected to be running under test; the point is that the scan works.
        let _ = running_game();
    }
}
