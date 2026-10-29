use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;

#[cfg(target_os = "linux")]
pub(super) fn run_in_terminal(command: &str) -> Result<(), String> {
    let terminal = std::env::var_os("TERMINAL");
    let mut process = linux_terminal_command(command, terminal.as_deref(), |program| {
        linux_program_exists(program)
    })?;
    let program = process.get_program().to_string_lossy().into_owned();
    process
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("failed to launch Linux terminal {program}: {error}"))
}

pub(super) const LINUX_TERMINALS: &[&str] = &[
    "x-terminal-emulator",
    "gnome-terminal",
    "kgx",
    "konsole",
    "xfce4-terminal",
    "alacritty",
    "kitty",
    "wezterm",
    "foot",
    "xterm",
];

#[derive(Debug, PartialEq, Eq)]
struct LinuxTerminal {
    executable: std::ffi::OsString,
    arguments: &'static [&'static str],
}

pub(super) fn linux_terminal_command<F>(
    command: &str,
    terminal_env: Option<&OsStr>,
    mut program_exists: F,
) -> Result<Command, String>
where
    F: FnMut(&OsStr) -> bool,
{
    let terminal = if let Some(value) = terminal_env.filter(|value| !value.is_empty()) {
        let terminal = linux_terminal(validate_terminal_env(value)?)?;
        if !program_exists(&terminal.executable) {
            return Err(format!(
                "terminal from $TERMINAL is not executable: {}; supported terminals: {}",
                Path::new(&terminal.executable).display(),
                LINUX_TERMINALS.join(", ")
            ));
        }
        terminal
    } else {
        LINUX_TERMINALS
            .iter()
            .copied()
            .map(std::ffi::OsString::from)
            .find(|executable| program_exists(executable))
            .map(linux_terminal)
            .transpose()?
            .ok_or_else(|| {
                format!(
                    "no supported Linux terminal found; install one or set $TERMINAL to one of: {}",
                    LINUX_TERMINALS.join(", ")
                )
            })?
    };

    let mut process = Command::new(terminal.executable);
    process
        .args(terminal.arguments)
        .arg("sh")
        .arg("-lc")
        .arg(command);
    Ok(process)
}

fn validate_terminal_env(value: &OsStr) -> Result<std::ffi::OsString, String> {
    let value = value
        .to_str()
        .ok_or_else(|| "$TERMINAL must contain valid UTF-8".to_owned())?;
    if !value.chars().all(|character| {
        character.is_ascii_alphanumeric() || matches!(character, '/' | '.' | '_' | '+' | '-')
    }) {
        return Err(
            "$TERMINAL must be an absolute path or a single executable name without arguments or shell metacharacters"
                .to_owned(),
        );
    }

    let path = Path::new(value);
    if !path.is_absolute() && value.contains('/') {
        return Err("$TERMINAL must be an absolute path or a single executable name".to_owned());
    }
    if path.file_name().is_none() {
        return Err("$TERMINAL must name a terminal executable".to_owned());
    }
    Ok(value.into())
}

fn linux_terminal(executable: std::ffi::OsString) -> Result<LinuxTerminal, String> {
    let name = Path::new(&executable)
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| "terminal executable name must contain valid UTF-8".to_owned())?;
    let arguments: &'static [&'static str] = match name {
        "x-terminal-emulator" | "konsole" | "alacritty" | "xterm" => &["-e"],
        "gnome-terminal" | "kgx" | "kitty" | "foot" => &["--"],
        "xfce4-terminal" => &["-x"],
        "wezterm" => &["start", "--"],
        _ => {
            return Err(format!(
                "unsupported terminal {name:?}; supported terminals: {}",
                LINUX_TERMINALS.join(", ")
            ));
        }
    };
    Ok(LinuxTerminal {
        executable,
        arguments,
    })
}

#[cfg(target_os = "linux")]
fn linux_program_exists(program: &OsStr) -> bool {
    use std::os::unix::fs::PermissionsExt;

    fn is_executable(path: &Path) -> bool {
        std::fs::metadata(path)
            .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
    }

    let path = Path::new(program);
    if path.is_absolute() {
        return is_executable(path);
    }
    std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|directory| is_executable(&directory.join(path)))
    })
}
