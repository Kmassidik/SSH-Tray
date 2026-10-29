use std::path::Path;

mod commands;
#[cfg(any(target_os = "linux", test))]
mod linux;
#[cfg(any(target_os = "macos", test))]
mod macos;

#[cfg(test)]
use commands::{
    change_key_passphrase_command, copy_id_command, generate_key_command, ssh_command,
    validate_host,
};
#[cfg(test)]
use linux::{LINUX_TERMINALS, linux_terminal_command};
#[cfg(test)]
use macos::{applescript_string, terminal_command_script};

/// Open a new desktop terminal window and run SSH for a validated host alias.
pub fn open_ssh(host: &str) -> Result<(), String> {
    run_in_terminal(&commands::ssh_command(host)?)
}

/// Open a desktop terminal and interactively generate an Ed25519 key pair.
///
/// No passphrase argument is supplied so ssh-keygen can prompt for it securely.
pub fn open_generate_key(path: &Path, comment: &str) -> Result<(), String> {
    run_in_terminal(&commands::generate_key_command(path, comment)?)
}

/// Open a desktop terminal and interactively change a private key's passphrase.
///
/// No passphrase argument is accepted or supplied; ssh-keygen handles secrets.
pub fn open_change_key_passphrase(private_key_path: &Path) -> Result<(), String> {
    run_in_terminal(&commands::change_key_passphrase_command(private_key_path)?)
}

/// Open a desktop terminal and interactively install a public key on a validated host.
pub fn open_copy_id(host: &str, public_key_path: &Path) -> Result<(), String> {
    run_in_terminal(&commands::copy_id_command(host, public_key_path)?)
}

#[cfg(target_os = "macos")]
fn run_in_terminal(command: &str) -> Result<(), String> {
    macos::run_in_terminal(command)
}

#[cfg(target_os = "linux")]
fn run_in_terminal(command: &str) -> Result<(), String> {
    linux::run_in_terminal(command)
}

#[cfg(test)]
fn terminal_script(host: &str) -> Result<String, String> {
    let command = ssh_command(host)?;
    Ok(terminal_command_script(&command))
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
