use std::ffi::OsStr;
use std::path::Path;

pub(super) fn ssh_command(host: &str) -> Result<String, String> {
    validate_host(host)?;
    // The allowlist above leaves no shell metacharacters to escape.
    Ok(format!("ssh {host}"))
}

pub(super) fn generate_key_command(path: &Path, comment: &str) -> Result<String, String> {
    let path = absolute_utf8_path(path, "private key path")?;
    Ok(format!(
        "ssh-keygen -t ed25519 -f {} -C {}",
        shell_single_quote(path),
        shell_single_quote(comment)
    ))
}

pub(super) fn change_key_passphrase_command(private_key_path: &Path) -> Result<String, String> {
    let path = absolute_utf8_path(private_key_path, "private key path")?;
    if private_key_path.extension() == Some(OsStr::new("pub")) {
        return Err(format!(
            "passphrase changes require a private key path, not a .pub file: {}",
            private_key_path.display()
        ));
    }
    Ok(format!("ssh-keygen -p -f {}", shell_single_quote(path)))
}

pub(super) fn copy_id_command(host: &str, public_key_path: &Path) -> Result<String, String> {
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

pub(super) fn validate_host(host: &str) -> Result<(), String> {
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
