// src/capture/window_finder.rs
use anyhow::{Result, anyhow};
use log::info;

pub struct WindowBounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[cfg(target_os = "windows")]
pub fn get_window_titles() -> Result<Vec<String>> {
    use windows::{
        core::PCWSTR,
        Win32::Foundation::{BOOL, HWND, LPARAM},
        Win32::UI::WindowsAndMessaging::{
            EnumWindows, GetWindowTextW, IsWindowVisible, WINDOW_STYLE
        },
    };
    
    info!("Finding window titles on Windows");
    let mut titles = Vec::new();
    
    unsafe {
        EnumWindows(
            Some(enum_window_proc),
            LPARAM(&mut titles as *mut Vec<String> as isize),
        )?;
    }
    
    Ok(titles)
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn enum_window_proc(
    hwnd: windows::Win32::Foundation::HWND,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::BOOL {
    use windows::{
        Win32::Foundation::TRUE,
        Win32::UI::WindowsAndMessaging::{GetWindowTextLengthW, GetWindowTextW, IsWindowVisible},
    };
    
    if IsWindowVisible(hwnd).as_bool() {
        let text_len = GetWindowTextLengthW(hwnd);
        if text_len > 0 {
            let mut buffer = vec![0u16; text_len as usize + 1];
            let len = GetWindowTextW(hwnd, &mut buffer);
            if len > 0 {
                buffer.truncate(len as usize);
                let title = String::from_utf16_lossy(&buffer);
                if !title.is_empty() {
                    let titles = &mut *(lparam.0 as *mut Vec<String>);
                    titles.push(title);
                }
            }
        }
    }
    
    TRUE
}

#[cfg(target_os = "linux")]
pub fn get_window_titles() -> Result<Vec<String>> {
    info!("Finding window titles on Linux");
    
    // Use the command-line tool to get window list
    let output = std::process::Command::new("xwininfo")
        .arg("-root")
        .arg("-tree")
        .output()?;
    
    let stdout = String::from_utf8(output.stdout)?;
    let titles: Vec<String> = stdout
        .lines()
        .filter_map(|line| {
            if line.contains("\"") {
                let start = line.find("\"");
                let end = line.rfind("\"");
                if let (Some(start), Some(end)) = (start, end) {
                    if start < end {
                        let title = &line[start + 1..end];
                        if !title.is_empty() {
                            return Some(title.to_string());
                        }
                    }
                }
            }
            None
        })
        .collect();
    
    Ok(titles)
}

#[cfg(target_os = "macos")]
pub fn get_window_titles() -> Result<Vec<String>> {
    info!("Finding window titles on macOS");
    
    // Use a command-line utility to get window list on macOS
    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg("tell application \"System Events\" to get name of every window of every process")
        .output()?;
    
    let stdout = String::from_utf8(output.stdout)?;
    let titles = stdout
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|title| !title.is_empty())
        .collect();
    
    Ok(titles)
}

#[cfg(target_os = "windows")]
pub fn get_window_bounds(window_title: &str) -> Result<WindowBounds> {
    use windows::{
        Win32::Foundation::{BOOL, HWND, LPARAM, RECT},
        Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowRect, GetWindowTextW},
    };
    
    info!("Getting window bounds for: {}", window_title);
    
    struct FindData {
        title: String,
        bounds: Option<WindowBounds>,
    }
    
    let mut find_data = FindData {
        title: window_title.to_string(),
        bounds: None,
    };
    
    unsafe {
        EnumWindows(
            Some(find_window_proc),
            LPARAM(&mut find_data as *mut FindData as isize),
        )?;
    }
    
    find_data.bounds.ok_or_else(|| anyhow!("Window not found: {}", window_title))
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn find_window_proc(
    hwnd: windows::Win32::Foundation::HWND,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::BOOL {
    use windows::{
        Win32::Foundation::{FALSE, RECT, TRUE},
        Win32::UI::WindowsAndMessaging::{GetWindowTextLengthW, GetWindowTextW, GetWindowRect, IsWindowVisible},
    };
    
    if IsWindowVisible(hwnd).as_bool() {
        let text_len = GetWindowTextLengthW(hwnd);
        if text_len > 0 {
            let mut buffer = vec![0u16; text_len as usize + 1];
            let len = GetWindowTextW(hwnd, &mut buffer);
            if len > 0 {
                buffer.truncate(len as usize);
                let title = String::from_utf16_lossy(&buffer);
                
                let find_data = &mut *(lparam.0 as *mut FindData);
                if title == find_data.title {
                    let mut rect = RECT::default();
                    if GetWindowRect(hwnd, &mut rect).is_ok() {
                        find_data.bounds = Some(WindowBounds {
                            x: rect.left,
                            y: rect.top,
                            width: rect.right - rect.left,
                            height: rect.bottom - rect.top,
                        });
                        return FALSE;
                    }
                }
            }
        }
    }
    
    TRUE
}

#[cfg(target_os = "linux")]
pub fn get_window_bounds(window_title: &str) -> Result<WindowBounds> {
    info!("Getting window bounds for: {}", window_title);
    
    // Use xwininfo to get window bounds
    let output = std::process::Command::new("xwininfo")
        .arg("-name")
        .arg(window_title)
        .output()?;
    
    let stdout = String::from_utf8(output.stdout)?;
    
    // Parse the xwininfo output
    let mut x = 0;
    let mut y = 0;
    let mut width = 0;
    let mut height = 0;
    
    for line in stdout.lines() {
        if line.contains("Absolute upper-left X:") {
            if let Some(val) = line.split(':').nth(1) {
                x = val.trim().parse::<i32>()?;
            }
        } else if line.contains("Absolute upper-left Y:") {
            if let Some(val) = line.split(':').nth(1) {
                y = val.trim().parse::<i32>()?;
            }
        } else if line.contains("Width:") {
            if let Some(val) = line.split(':').nth(1) {
                width = val.trim().parse::<i32>()?;
            }
        } else if line.contains("Height:") {
            if let Some(val) = line.split(':').nth(1) {
                height = val.trim().parse::<i32>()?;
            }
        }
    }
    
    if width == 0 || height == 0 {
        return Err(anyhow!("Window not found or has invalid dimensions: {}", window_title));
    }
    
    Ok(WindowBounds { x, y, width, height })
}

#[cfg(target_os = "macos")]
pub fn get_window_bounds(window_title: &str) -> Result<WindowBounds> {
    info!("Getting window bounds for: {}", window_title);
    
    // AppleScript to get window bounds
    let script = format!(
        r#"
        tell application "System Events"
            set targetWindow to first window of first application process whose name contains "{}"
            set pos to position of targetWindow
            set dims to size of targetWindow
            return {{item 1 of pos, item 2 of pos, item 1 of dims, item 2 of dims}}
        end tell
        "#,
        window_title
    );
    
    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()?;
    
    let stdout = String::from_utf8(output.stdout)?;
    
    // Parse the AppleScript output, which is a list like "{x, y, width, height}"
    let values: Vec<i32> = stdout
        .trim()
        .trim_matches(|c| c == '{' || c == '}')
        .split(',')
        .filter_map(|s| s.trim().parse::<i32>().ok())
        .collect();
    
    if values.len() != 4 {
        return Err(anyhow!("Failed to get window bounds for: {}", window_title));
    }
    
    Ok(WindowBounds {
        x: values[0],
        y: values[1],
        width: values[2],
        height: values[3],
    })
}