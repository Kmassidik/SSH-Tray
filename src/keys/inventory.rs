use super::PublicKey;
use std::ffi::OsStr;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::process::Command;

pub(super) fn load_public_keys_in(dir: &Path) -> Result<Vec<PublicKey>, String> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("failed to read {}: {error}", dir.display())),
    };

    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("failed to read {}: {error}", dir.display()))?;
        let path = entry.path();
        if path.extension() != Some(OsStr::new("pub")) {
            continue;
        }
        let file_type = entry
            .file_type()
            .map_err(|error| format!("failed to inspect {}: {error}", path.display()))?;
        if file_type.is_file() {
            paths.push(path);
        }
    }
    paths.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    paths.iter().map(|path| read_public_key(path)).collect()
}

pub(super) fn read_public_key(path: &Path) -> Result<PublicKey, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read public key {}: {error}", path.display()))?;
    let contents = contents.trim_end().to_owned();
    let (key_type, comment) = parse_public_key(&contents)
        .map_err(|error| format!("invalid public key {}: {error}", path.display()))?;
    let output = Command::new("ssh-keygen")
        .arg("-lf")
        .arg(path)
        .output()
        .map_err(|error| format!("failed to run ssh-keygen for {}: {error}", path.display()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "ssh-keygen could not fingerprint {}: {}{}",
            path.display(),
            output.status,
            if stderr.trim().is_empty() {
                String::new()
            } else {
                format!(": {}", stderr.trim())
            }
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let fingerprint = stdout
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| format!("ssh-keygen returned no fingerprint for {}", path.display()))?
        .to_owned();
    let name = path
        .file_name()
        .ok_or_else(|| format!("public key path has no filename: {}", path.display()))?
        .to_string_lossy()
        .into_owned();

    Ok(PublicKey {
        path: path.to_path_buf(),
        name,
        key_type,
        comment,
        fingerprint,
        contents,
    })
}

pub(super) fn parse_public_key(contents: &str) -> Result<(String, String), String> {
    let mut fields = contents.split_whitespace();
    let key_type = fields.next().ok_or_else(|| "key is empty".to_owned())?;
    fields
        .next()
        .ok_or_else(|| "key data is missing".to_owned())?;
    Ok((key_type.to_owned(), fields.collect::<Vec<_>>().join(" ")))
}

pub(super) fn update_public_key_comment(
    key: &PublicKey,
    comment: &str,
) -> Result<PublicKey, String> {
    let contents = fs::read_to_string(&key.path)
        .map_err(|error| format!("failed to read public key {}: {error}", key.path.display()))?;
    let updated = replace_public_key_comment(&contents, comment)?;
    fs::write(&key.path, updated).map_err(|error| {
        format!(
            "failed to update public key {}: {error}",
            key.path.display()
        )
    })?;
    read_public_key(&key.path)
}

pub(super) fn replace_public_key_comment(contents: &str, comment: &str) -> Result<String, String> {
    if comment.contains(['\r', '\n']) {
        return Err("public key comment must be a single line".to_owned());
    }

    let (line, ending) = if let Some(line) = contents.strip_suffix("\r\n") {
        (line, "\r\n")
    } else if let Some(line) = contents.strip_suffix('\n') {
        (line, "\n")
    } else if let Some(line) = contents.strip_suffix('\r') {
        (line, "\r")
    } else {
        (contents, "")
    };
    if line.contains(['\r', '\n']) {
        return Err("public key must contain exactly one line".to_owned());
    }

    let mut fields = line.split_whitespace();
    let key_type = fields.next().ok_or_else(|| "key is empty".to_owned())?;
    let key_data = fields
        .next()
        .ok_or_else(|| "key data is missing".to_owned())?;
    let key_type_start = line
        .find(key_type)
        .ok_or_else(|| "key type is missing".to_owned())?;
    let after_key_type = key_type_start + key_type.len();
    let key_data_start = after_key_type
        + line[after_key_type..]
            .find(key_data)
            .ok_or_else(|| "key data is missing".to_owned())?;
    let key_data_end = key_data_start + key_data.len();

    let mut updated = String::with_capacity(key_data_end + comment.len() + ending.len() + 1);
    updated.push_str(&line[..key_data_end]);
    if !comment.is_empty() {
        updated.push(' ');
        updated.push_str(comment);
    }
    updated.push_str(ending);
    Ok(updated)
}
