#[cfg(target_os = "macos")]
pub(super) fn run_in_terminal(command: &str) -> Result<(), String> {
    run_osascript(terminal_command_script(command))
}

#[cfg(target_os = "macos")]
fn run_osascript(script: String) -> Result<(), String> {
    use std::process::Command;

    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|error| format!("failed to launch osascript: {error}"))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let detail = stderr.trim();
    if detail.is_empty() {
        Err(format!(
            "osascript failed with {} and no error output",
            output.status
        ))
    } else {
        Err(format!("osascript failed with {}: {detail}", output.status))
    }
}

pub(super) fn terminal_command_script(command: &str) -> String {
    format!(
        "tell application \"Terminal\"\n    activate\n    do script {}\nend tell",
        applescript_string(command)
    )
}

pub(super) fn applescript_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for character in value.chars() {
        match character {
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            '"' | '\\' => {
                escaped.push('\\');
                escaped.push(character);
            }
            _ => escaped.push(character),
        }
    }
    escaped.push('"');
    escaped
}
