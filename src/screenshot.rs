//! Window screenshot capture and input simulation via PowerShell (Windows only)

use std::path::Path;
use std::process::Command;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Lifecycle test scenario
#[derive(Debug, serde::Deserialize)]
pub struct Scenario {
    pub exe: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub steps: Vec<Step>,
}

/// A single step in a lifecycle test
#[derive(Debug, serde::Deserialize)]
#[serde(tag = "action")]
pub enum Step {
    #[serde(rename = "wait")]
    Wait { seconds: u32 },
    #[serde(rename = "screenshot")]
    Screenshot { expect: String },
    #[serde(rename = "key")]
    Key {
        keys: String,
        #[serde(default)]
        comment: String,
    },
}

/// Capture a screenshot of the main window of the given process name.
/// Saves as PNG to output_path.
pub fn capture_window(process_name: &str, output_path: &Path) -> Result<(), String> {
    let output_str = output_path.to_string_lossy().replace('\\', "\\\\");
    let ps_script = format!(
        r#"Add-Type -AssemblyName System.Windows.Forms,System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Win32 {{
    [DllImport("user32.dll")]
    public static extern bool GetWindowRect(IntPtr hWnd, out RECT lpRect);
    [DllImport("user32.dll")]
    public static extern bool SetForegroundWindow(IntPtr hWnd);
}}
public struct RECT {{
    public int Left, Top, Right, Bottom;
}}
"@
$proc = Get-Process -Name '{process_name}' -ErrorAction Stop | Where-Object {{ $_.MainWindowHandle -ne 0 }} | Select-Object -First 1
if (-not $proc) {{ throw "No window found for {process_name}" }}
$hwnd = $proc.MainWindowHandle
[Win32]::SetForegroundWindow($hwnd) | Out-Null
Start-Sleep -Milliseconds 500
$rect = New-Object RECT
[Win32]::GetWindowRect($hwnd, [ref]$rect) | Out-Null
$w = $rect.Right - $rect.Left
$h = $rect.Bottom - $rect.Top
$bmp = New-Object System.Drawing.Bitmap($w, $h)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($rect.Left, $rect.Top, 0, 0, (New-Object System.Drawing.Size($w, $h)))
$g.Dispose()
$bmp.Save('{output_str}', [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()
Write-Output "OK""#,
        process_name = process_name,
        output_str = output_str,
    );

    let mut cmd = Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", &ps_script]);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let output = cmd
        .output()
        .map_err(|e| format!("PowerShell execution failed: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Screenshot failed: {}", stderr));
    }

    if !output_path.exists() {
        return Err("Screenshot file was not created".to_string());
    }

    Ok(())
}

/// Send keystrokes to the foreground window via PowerShell SendKeys.
/// `keys` uses PowerShell SendKeys syntax: "^t" = Ctrl+T, "%{F4}" = Alt+F4, etc.
/// Common mappings: ctrl=^, alt=%, shift=+, {ENTER}, {TAB}, {ESC}, {F1}-{F12}
pub fn send_keys(process_name: &str, keys: &str) -> Result<(), String> {
    // Convert human-readable "ctrl+t" to SendKeys "^t" format
    let send_keys_str = convert_to_sendkeys(keys);

    let ps_script = format!(
        r#"Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Win32Keys {{
    [DllImport("user32.dll")]
    public static extern bool SetForegroundWindow(IntPtr hWnd);
}}
"@
$proc = Get-Process -Name '{process_name}' -ErrorAction Stop | Where-Object {{ $_.MainWindowHandle -ne 0 }} | Select-Object -First 1
if (-not $proc) {{ throw "No window found for {process_name}" }}
[Win32Keys]::SetForegroundWindow($proc.MainWindowHandle) | Out-Null
Start-Sleep -Milliseconds 200
Add-Type -AssemblyName System.Windows.Forms
[System.Windows.Forms.SendKeys]::SendWait('{send_keys_str}')
Write-Output "OK""#,
        process_name = process_name,
        send_keys_str = send_keys_str,
    );

    let mut cmd = Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", &ps_script]);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let output = cmd
        .output()
        .map_err(|e| format!("SendKeys execution failed: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("SendKeys failed: {}", stderr));
    }

    Ok(())
}

/// Convert human-readable key notation to PowerShell SendKeys format.
/// "ctrl+t" → "^t", "ctrl+shift+t" → "^+t", "alt+F4" → "%{F4}", "enter" → "{ENTER}"
fn convert_to_sendkeys(keys: &str) -> String {
    let parts: Vec<&str> = keys.split('+').collect();
    if parts.len() == 1 {
        // Single key
        return match parts[0].to_lowercase().as_str() {
            "enter" | "return" => "{ENTER}".to_string(),
            "tab" => "{TAB}".to_string(),
            "esc" | "escape" => "{ESC}".to_string(),
            "space" => " ".to_string(),
            "delete" | "del" => "{DEL}".to_string(),
            "backspace" => "{BACKSPACE}".to_string(),
            k if k.starts_with('f') && k[1..].parse::<u32>().is_ok() => {
                format!("{{{}}}", k.to_uppercase())
            }
            k => k.to_string(),
        };
    }

    let mut prefix = String::new();
    let mut main_key = String::new();

    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            // Last part is the main key
            main_key = match part.to_lowercase().as_str() {
                k if k.starts_with('f') && k[1..].parse::<u32>().is_ok() => {
                    format!("{{{}}}", k.to_uppercase())
                }
                "enter" | "return" => "{ENTER}".to_string(),
                "tab" => "{TAB}".to_string(),
                "esc" | "escape" => "{ESC}".to_string(),
                "delete" | "del" => "{DEL}".to_string(),
                k => k.to_lowercase(),
            };
        } else {
            // Modifier
            match part.to_lowercase().as_str() {
                "ctrl" | "control" => prefix.push('^'),
                "alt" => prefix.push('%'),
                "shift" => prefix.push('+'),
                _ => {}
            }
        }
    }

    format!("{}{}", prefix, main_key)
}
