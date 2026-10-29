use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;

/// Open a new desktop terminal window and run SSH for a validated host alias.
pub fn open_ssh(host: &str) -> Result<(), String> {
    run_in_terminal(&ssh_command(host)?)
}

/// Open a desktop terminal and interactively generate an Ed25519 key pair.
///
/// No passphrase argument is supplied so ssh-keygen can prompt for it securely.
pub fn open_generate_key(path: &Path, comment: &str) -> Result<(), String> {
    run_in_terminal(&generate_key_command(path, comment)?)
}

/// Open a desktop terminal and interactively change a private key's passphrase.
///
/// No passphrase argument is accepted or supplied; ssh-keygen handles secrets.
pub fn open_change_key_passphrase(private_key_path: &Path) -> Result<(), String> {
    run_in_terminal(&change_key_passphrase_command(private_key_path)?)
}

/// Open a desktop terminal and interactively install a public key on a validated host.
pub fn open_copy_id(host: &str, public_key_path: &Path) -> Result<(), String> {
    run_in_terminal(&copy_id_command(host, public_key_path)?)
}

#[cfg(target_os = "macos")]
fn run_in_terminal(command: &str) -> Result<(), String> {
    run_osascript(terminal_command_script(command))
}

#[cfg(target_os = "linux")]
fn run_in_terminal(command: &str) -> Result<(), String> {
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

#[cfg(target_os = "macos")]
fn run_osascript(script: String) -> Result<(), String> {
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

#[cfg(any(target_os = "linux", test))]
const LINUX_TERMINALS: &[&str] = &[
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

#[cfg(any(target_os = "linux", test))]
#[derive(Debug, PartialEq, Eq)]
struct LinuxTerminal {
    executable: std::ffi::OsString,
    arguments: &'static [&'static str],
}

#[cfg(any(target_os = "linux", test))]
fn linux_terminal_command<F>(
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

#[cfg(any(target_os = "linux", test))]
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

#[cfg(any(target_os = "linux", test))]
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

#[cfg(test)]
fn terminal_script(host: &str) -> Result<String, String> {
    let command = ssh_command(host)?;
    Ok(terminal_command_script(&command))
}

#[cfg(any(target_os = "macos", test))]
fn terminal_command_script(command: &str) -> String {
    format!(
        "tell application \"Terminal\"\n    activate\n    do script {}\nend tell",
        applescript_string(command)
    )
}

fn ssh_command(host: &str) -> Result<String, String> {
    validate_host(host)?;
    // The allowlist above leaves no shell metacharacters to escape.
    Ok(format!("ssh {host}"))
}

fn generate_key_command(path: &Path, comment: &str) -> Result<String, String> {
    let path = absolute_utf8_path(path, "private key path")?;
    Ok(format!(
        "ssh-keygen -t ed25519 -f {} -C {}",
        shell_single_quote(path),
        shell_single_quote(comment)
    ))
}

fn change_key_passphrase_command(private_key_path: &Path) -> Result<String, String> {
    let path = absolute_utf8_path(private_key_path, "private key path")?;
    if private_key_path.extension() == Some(OsStr::new("pub")) {
        return Err(format!(
            "passphrase changes require a private key path, not a .pub file: {}",
            private_key_path.display()
        ));
    }
    Ok(format!("ssh-keygen -p -f {}", shell_single_quote(path)))
}

fn copy_id_command(host: &str, public_key_path: &Path) -> Result<String, String> {
    validate_host(host)?;
    let path = absolute_utf8_path(public_key_path, "public key path")?;
    if public_key_path.extension() != Some(OsStr::new("pub")) {
        return Err(format!(
            "public key path must have a .pub extension: {}",
            public_key_path.display()
        ));
    }
    Ok(format!(
        "ssh-copy-id -i {} {host}",
        shell_single_quote(path)
    ))
}

fn absolute_utf8_path<'a>(path: &'a Path, description: &str) -> Result<&'a str, String> {
    if path.as_os_str().is_empty() {
        return Err(format!("{description} cannot be empty"));
    }
    if !path.is_absolute() {
        return Err(format!(
            "{description} must be absolute: {}",
            path.display()
        ));
    }
    path.to_str()
        .ok_or_else(|| format!("{description} must contain valid UTF-8: {}", path.display()))
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn validate_host(host: &str) -> Result<(), String> {
    if host.is_empty() {
        return Err("SSH host alias cannot be empty".to_owned());
    }
    if host.starts_with('-') {
        return Err("SSH host alias cannot start with '-'".to_owned());
    }
    if !host
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-'))
    {
        return Err(format!(
            "unsafe SSH host alias {host:?}: only ASCII letters, digits, '.', '_' and '-' are allowed"
        ));
    }
    Ok(())
}

#[cfg(any(target_os = "macos", test))]
fn applescript_string(value: &str) -> String {
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

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::path::Path;

    use super::{
        LINUX_TERMINALS, applescript_string, change_key_passphrase_command, copy_id_command,
        generate_key_command, linux_terminal_command, ssh_command, terminal_command_script,
        terminal_script, validate_host,
    };

    #[test]
    fn accepts_concrete_host_aliases() {
        for alias in [
            "server",
            "dev-box",
            "dev_box",
            "host.example.com",
            "Host123",
        ] {
            assert_eq!(ssh_command(alias).unwrap(), format!("ssh {alias}"));
        }
    }

    #[test]
    fn builds_a_new_terminal_window_script() {
        assert_eq!(
            terminal_script("dev-box").unwrap(),
            "tell application \"Terminal\"\n    activate\n    do script \"ssh dev-box\"\nend tell"
        );
    }

    #[test]
    fn rejects_empty_and_option_like_aliases() {
        for alias in ["", "-v", "-oProxyCommand=whoami"] {
            assert!(validate_host(alias).is_err(), "accepted {alias:?}");
        }
    }

    #[test]
    fn rejects_shell_and_applescript_injection() {
        for alias in [
            "host; whoami",
            "host && whoami",
            "host|whoami",
            "$(whoami)",
            "`whoami`",
            "host\"\ndo script \"whoami",
            "host\\whoami",
            "host\nwhoami",
            "host whoami",
            "host'whoami",
            "host*",
        ] {
            assert!(terminal_script(alias).is_err(), "accepted {alias:?}");
        }
    }

    #[test]
    fn builds_exact_interactive_key_commands() {
        assert_eq!(
            generate_key_command(Path::new("/tmp/id_ed25519"), "alice@example.com").unwrap(),
            "ssh-keygen -t ed25519 -f '/tmp/id_ed25519' -C 'alice@example.com'"
        );
        assert_eq!(
            copy_id_command("dev-box", Path::new("/tmp/id_ed25519.pub")).unwrap(),
            "ssh-copy-id -i '/tmp/id_ed25519.pub' dev-box"
        );
        assert_eq!(
            change_key_passphrase_command(Path::new("/tmp/id_ed25519")).unwrap(),
            "ssh-keygen -p -f '/tmp/id_ed25519'"
        );
    }

    #[test]
    fn quotes_malicious_key_paths_and_comments() {
        let command = generate_key_command(
            Path::new("/tmp/key'; touch /tmp/pwned; #"),
            "alice'; $(touch /tmp/pwned); #",
        )
        .unwrap();
        assert_eq!(
            command,
            r#"ssh-keygen -t ed25519 -f '/tmp/key'\''; touch /tmp/pwned; #' -C 'alice'\''; $(touch /tmp/pwned); #'"#
        );

        let command =
            copy_id_command("dev-box", Path::new("/tmp/id';touch /tmp/pwned;#.pub")).unwrap();
        assert_eq!(
            command,
            r#"ssh-copy-id -i '/tmp/id'\'';touch /tmp/pwned;#.pub' dev-box"#
        );

        let command =
            change_key_passphrase_command(Path::new("/tmp/key'; touch /tmp/pwned; #")).unwrap();
        assert_eq!(
            command,
            r#"ssh-keygen -p -f '/tmp/key'\''; touch /tmp/pwned; #'"#
        );
    }

    #[test]
    fn escapes_malicious_comments_for_applescript() {
        let command =
            generate_key_command(Path::new("/tmp/key"), "alice\"\ndo script \"whoami").unwrap();
        assert_eq!(
            terminal_command_script(&command),
            concat!(
                "tell application \"Terminal\"\n",
                "    activate\n",
                "    do script \"ssh-keygen -t ed25519 -f '/tmp/key' -C 'alice\\\"\\n",
                "do script \\\"whoami'\"\n",
                "end tell"
            )
        );
    }

    #[test]
    fn validates_key_paths_and_copy_id_hosts() {
        for path in ["", "relative/key", "tmp/key"] {
            assert!(generate_key_command(Path::new(path), "comment").is_err());
        }
        for path in ["", "relative/key.pub", "/tmp/key", "/tmp/key.PUB"] {
            assert!(copy_id_command("dev-box", Path::new(path)).is_err());
        }
        for path in ["", "relative/key", "/tmp/key.pub"] {
            assert!(
                change_key_passphrase_command(Path::new(path)).is_err(),
                "accepted {path:?}"
            );
        }
        assert!(copy_id_command("host;whoami", Path::new("/tmp/id_ed25519.pub")).is_err());
    }

    #[test]
    fn selects_correct_linux_terminal_argv() {
        let cases: &[(&str, &[&str])] = &[
            ("x-terminal-emulator", &["-e"]),
            ("gnome-terminal", &["--"]),
            ("kgx", &["--"]),
            ("konsole", &["-e"]),
            ("xfce4-terminal", &["-x"]),
            ("alacritty", &["-e"]),
            ("kitty", &["--"]),
            ("wezterm", &["start", "--"]),
            ("foot", &["--"]),
            ("xterm", &["-e"]),
        ];

        for &(name, prefix) in cases {
            let process =
                linux_terminal_command("ssh dev-box", Some(OsStr::new(name)), |program| {
                    program == OsStr::new(name)
                })
                .unwrap();
            assert_eq!(process.get_program(), OsStr::new(name));
            let actual: Vec<&str> = process
                .get_args()
                .map(|argument| argument.to_str().unwrap())
                .collect();
            let expected: Vec<&str> = prefix
                .iter()
                .copied()
                .chain(["sh", "-lc", "ssh dev-box"])
                .collect();
            assert_eq!(actual, expected, "wrong argv for {name}");
        }
    }

    #[test]
    fn linux_terminal_env_accepts_absolute_paths_and_takes_precedence() {
        let process = linux_terminal_command(
            "ssh dev-box",
            Some(OsStr::new("/opt/kitty/bin/kitty")),
            |program| {
                program == OsStr::new("/opt/kitty/bin/kitty")
                    || program == OsStr::new("x-terminal-emulator")
            },
        )
        .unwrap();
        assert_eq!(process.get_program(), OsStr::new("/opt/kitty/bin/kitty"));
        assert_eq!(process.get_args().next(), Some(OsStr::new("--")));
    }

    #[test]
    fn linux_terminal_detection_uses_supported_fallback_order() {
        let process = linux_terminal_command("ssh dev-box", None, |program| {
            program == OsStr::new("foot") || program == OsStr::new("xterm")
        })
        .unwrap();
        assert_eq!(process.get_program(), OsStr::new("foot"));

        let process = linux_terminal_command("ssh dev-box", Some(OsStr::new("")), |_| false);
        let error = match process {
            Ok(_) => panic!("found a terminal without any executable"),
            Err(error) => error,
        };
        for terminal in LINUX_TERMINALS {
            assert!(
                error.contains(terminal),
                "error omitted {terminal}: {error}"
            );
        }
    }

    #[test]
    fn rejects_unsafe_or_unsupported_terminal_env_values() {
        for value in [
            "kitty --hold",
            "kitty; touch /tmp/pwned",
            "kitty $(touch /tmp/pwned)",
            "kitty`touch /tmp/pwned`",
            "./kitty",
            "bin/kitty",
            "/",
            "unsupported-terminal",
        ] {
            let result = linux_terminal_command("ssh dev-box", Some(OsStr::new(value)), |_| true);
            assert!(result.is_err(), "accepted $TERMINAL value {value:?}");
        }

        let error = linux_terminal_command(
            "ssh dev-box",
            Some(OsStr::new("unsupported-terminal")),
            |_| true,
        )
        .unwrap_err();
        assert!(error.contains("supported terminals"));
    }

    #[test]
    fn every_linux_action_command_is_one_shell_argument() {
        let commands = [
            ssh_command("dev-box").unwrap(),
            generate_key_command(
                Path::new("/tmp/key'; touch /tmp/pwned; #"),
                "alice'; $(touch /tmp/pwned); #",
            )
            .unwrap(),
            change_key_passphrase_command(Path::new("/tmp/key'; touch /tmp/pwned; #")).unwrap(),
            copy_id_command("dev-box", Path::new("/tmp/id'; touch /tmp/pwned; #.pub")).unwrap(),
        ];

        for command in commands {
            let process =
                linux_terminal_command(&command, Some(OsStr::new("xterm")), |_| true).unwrap();
            let arguments: Vec<&OsStr> = process.get_args().collect();
            assert_eq!(
                arguments,
                [
                    OsStr::new("-e"),
                    OsStr::new("sh"),
                    OsStr::new("-lc"),
                    OsStr::new(&command)
                ]
            );
        }
    }

    #[test]
    fn escapes_applescript_string_delimiters() {
        assert_eq!(
            applescript_string(r#"say \"hello\" \\ goodbye"#),
            r#""say \\\"hello\\\" \\\\ goodbye""#
        );
    }
}
