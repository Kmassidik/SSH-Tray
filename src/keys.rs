use std::env;
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicKey {
    pub path: PathBuf,
    pub name: String,
    pub key_type: String,
    pub comment: String,
    pub fingerprint: String,
    pub contents: String,
}

pub fn load_public_keys() -> Result<Vec<PublicKey>, String> {
    load_public_keys_in(&ssh_dir()?)
}

pub fn key_path(file_name: &str) -> Result<PathBuf, String> {
    key_path_in(&ssh_dir()?, file_name)
}

pub fn import_key_pair(source: &Path) -> Result<PublicKey, String> {
    import_key_pair_in(&ssh_dir()?, source)
}

pub fn remove_key_pair(key: &PublicKey) -> Result<(), String> {
    remove_pair_files(&key.path)
}

/// Replace only the comment in a public key file and return refreshed metadata.
pub fn update_public_key_comment(key: &PublicKey, comment: &str) -> Result<PublicKey, String> {
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

fn ssh_dir() -> Result<PathBuf, String> {
    let home =
        env::var_os("HOME").ok_or_else(|| "HOME is not set; cannot locate ~/.ssh".to_owned())?;
    Ok(PathBuf::from(home).join(".ssh"))
}

fn load_public_keys_in(dir: &Path) -> Result<Vec<PublicKey>, String> {
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

fn read_public_key(path: &Path) -> Result<PublicKey, String> {
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

fn parse_public_key(contents: &str) -> Result<(String, String), String> {
    let mut fields = contents.split_whitespace();
    let key_type = fields.next().ok_or_else(|| "key is empty".to_owned())?;
    fields
        .next()
        .ok_or_else(|| "key data is missing".to_owned())?;
    Ok((key_type.to_owned(), fields.collect::<Vec<_>>().join(" ")))
}

fn replace_public_key_comment(contents: &str, comment: &str) -> Result<String, String> {
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

fn key_path_in(dir: &Path, file_name: &str) -> Result<PathBuf, String> {
    let path = Path::new(file_name);
    if file_name.is_empty()
        || file_name.ends_with(".pub")
        || path.file_name() != Some(OsStr::new(file_name))
        || file_name == "."
        || file_name == ".."
    {
        return Err(format!("invalid private key filename: {file_name:?}"));
    }
    Ok(dir.join(file_name))
}

fn import_key_pair_in(dir: &Path, source: &Path) -> Result<PublicKey, String> {
    let file_name = source.file_name().and_then(OsStr::to_str).ok_or_else(|| {
        format!(
            "private key path must have a UTF-8 filename: {}",
            source.display()
        )
    })?;
    let destination_private = key_path_in(dir, file_name)?;
    let public_name = format!("{file_name}.pub");
    let source_public = source.with_file_name(&public_name);
    let destination_public = dir.join(&public_name);

    ensure_regular_file(source, "private key")?;
    ensure_regular_file(&source_public, "public key")?;
    if same_file(source, &destination_private) || same_file(&source_public, &destination_public) {
        return Err(format!(
            "{} is already in {}",
            source.display(),
            dir.display()
        ));
    }

    // Fingerprint before copying so a command failure cannot leave a half-imported pair.
    let mut key = read_public_key(&source_public)?;
    copy_key_pair(
        source,
        &source_public,
        &destination_private,
        &destination_public,
    )?;
    key.path = destination_public;
    Ok(key)
}

fn ensure_regular_file(path: &Path, kind: &str) -> Result<(), String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("failed to access {kind} {}: {error}", path.display()))?;
    if metadata.is_file() {
        Ok(())
    } else {
        Err(format!("{kind} is not a regular file: {}", path.display()))
    }
}

fn same_file(a: &Path, b: &Path) -> bool {
    a == b
        || match (fs::canonicalize(a), fs::canonicalize(b)) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        }
}

fn copy_key_pair(
    source_private: &Path,
    source_public: &Path,
    destination_private: &Path,
    destination_public: &Path,
) -> Result<(), String> {
    copy_file(source_private, destination_private, 0o600)?;
    if let Err(error) = copy_file(source_public, destination_public, 0o644) {
        return Err(match fs::remove_file(destination_private) {
            Ok(()) => error,
            Err(cleanup) if cleanup.kind() == ErrorKind::NotFound => error,
            Err(cleanup) => format!(
                "{error}; also failed to remove private key copy {}: {cleanup}",
                destination_private.display()
            ),
        });
    }
    Ok(())
}

fn copy_file(source: &Path, destination: &Path, mode: u32) -> Result<(), String> {
    let mut source_file = File::open(source)
        .map_err(|error| format!("failed to open {}: {error}", source.display()))?;
    let mut destination_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(destination)
        .map_err(|error| {
            if error.kind() == ErrorKind::AlreadyExists {
                format!(
                    "refusing to overwrite existing key {}",
                    destination.display()
                )
            } else {
                format!("failed to create {}: {error}", destination.display())
            }
        })?;

    let result = io::copy(&mut source_file, &mut destination_file)
        .map(|_| ())
        .and_then(|()| destination_file.set_permissions(fs::Permissions::from_mode(mode)));
    if let Err(error) = result {
        let message = format!(
            "failed to copy {} to {}: {error}",
            source.display(),
            destination.display()
        );
        return Err(match fs::remove_file(destination) {
            Ok(()) => message,
            Err(cleanup) if cleanup.kind() == ErrorKind::NotFound => message,
            Err(cleanup) => format!("{message}; also failed to remove partial copy: {cleanup}"),
        });
    }
    Ok(())
}

fn remove_pair_files(public_path: &Path) -> Result<(), String> {
    let name = public_path
        .file_name()
        .and_then(OsStr::to_str)
        .and_then(|name| name.strip_suffix(".pub"))
        .filter(|name| !name.is_empty())
        .ok_or_else(|| format!("invalid public key path: {}", public_path.display()))?;
    let private_path = public_path.with_file_name(name);
    let mut errors = Vec::new();
    for path in [public_path, private_path.as_path()] {
        if let Err(error) = fs::remove_file(path) {
            errors.push(format!("failed to remove {}: {error}", path.display()));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_DIR: AtomicUsize = AtomicUsize::new(0);

    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Self {
            let id = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
            let path = env::temp_dir().join(format!("ssh-tray-keys-{}-{id}", std::process::id()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn join(&self, path: &str) -> PathBuf {
            self.0.join(path)
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn parses_type_and_comment_without_changing_contents() {
        let parsed = parse_public_key("ssh-ed25519 AAAAC3NzaC user@example.com\n").unwrap();
        assert_eq!(parsed.0, "ssh-ed25519");
        assert_eq!(parsed.1, "user@example.com");
        assert!(parse_public_key("ssh-ed25519").is_err());
        assert!(parse_public_key("").is_err());
    }

    #[test]
    fn comment_replacement_preserves_key_fields_and_line_ending() {
        let contents = "ssh-ed25519   AAAAC3NzaC1lZDI1NTE5AAAAIAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA  old comment\r\n";
        let updated = replace_public_key_comment(contents, "new comment").unwrap();
        assert_eq!(
            updated,
            "ssh-ed25519   AAAAC3NzaC1lZDI1NTE5AAAAIAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA new comment\r\n"
        );
        assert_eq!(
            replace_public_key_comment(contents, "").unwrap(),
            "ssh-ed25519   AAAAC3NzaC1lZDI1NTE5AAAAIAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\r\n"
        );
    }

    #[test]
    fn comment_update_preserves_permissions_and_refreshes_metadata() {
        let temp = TestDir::new();
        let path = temp.join("id_ed25519.pub");
        let key_data = "AAAAC3NzaC1lZDI1NTE5AAAAIG089ylLXpZ2s8JVPIYiuypLT0ShbvP265xGVZHY2V/Y";
        fs::write(&path, format!("ssh-ed25519 {key_data} old@example.com\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        let key = PublicKey {
            path: path.clone(),
            name: String::new(),
            key_type: String::new(),
            comment: String::new(),
            fingerprint: String::new(),
            contents: String::new(),
        };

        let updated = update_public_key_comment(&key, "new@example.com").unwrap();

        assert_eq!(updated.path, path);
        assert_eq!(updated.name, "id_ed25519.pub");
        assert_eq!(updated.key_type, "ssh-ed25519");
        assert_eq!(updated.comment, "new@example.com");
        assert_eq!(
            updated.contents,
            format!("ssh-ed25519 {key_data} new@example.com")
        );
        assert!(!updated.fingerprint.is_empty());
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            format!("ssh-ed25519 {key_data} new@example.com\n")
        );
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o640
        );
    }

    #[test]
    fn multiline_comments_are_rejected_without_modifying_the_key() {
        let temp = TestDir::new();
        let path = temp.join("id.pub");
        let original = "ssh-ed25519 AAAA old@example.com\n";
        fs::write(&path, original).unwrap();
        let key = PublicKey {
            path: path.clone(),
            name: String::new(),
            key_type: String::new(),
            comment: String::new(),
            fingerprint: String::new(),
            contents: String::new(),
        };

        for comment in ["first\nsecond", "first\rsecond", "first\r\nsecond"] {
            assert!(update_public_key_comment(&key, comment).is_err());
            assert_eq!(fs::read_to_string(&path).unwrap(), original);
        }
    }

    #[test]
    fn private_key_paths_only_accept_simple_non_public_names() {
        let dir = Path::new("/isolated/.ssh");
        assert_eq!(
            key_path_in(dir, "id_ed25519").unwrap(),
            dir.join("id_ed25519")
        );
        for name in ["", ".", "..", "id.pub", "../id", "other/id", "./id"] {
            assert!(key_path_in(dir, name).is_err(), "accepted {name:?}");
        }
    }

    #[test]
    fn copies_pairs_with_private_permissions() {
        let temp = TestDir::new();
        let source = temp.join("source");
        let destination = temp.join("destination");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&destination).unwrap();
        fs::write(source.join("id"), b"private placeholder").unwrap();
        fs::write(source.join("id.pub"), b"public placeholder").unwrap();

        copy_key_pair(
            &source.join("id"),
            &source.join("id.pub"),
            &destination.join("id"),
            &destination.join("id.pub"),
        )
        .unwrap();

        assert_eq!(
            fs::read(destination.join("id")).unwrap(),
            b"private placeholder"
        );
        assert_eq!(
            fs::read(destination.join("id.pub")).unwrap(),
            b"public placeholder"
        );
        assert_eq!(
            fs::metadata(destination.join("id"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(destination.join("id.pub"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o644
        );
    }

    #[test]
    fn copy_never_overwrites_and_rolls_back_private_copy() {
        let temp = TestDir::new();
        let source = temp.join("source");
        let destination = temp.join("destination");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&destination).unwrap();
        fs::write(source.join("id"), b"new private").unwrap();
        fs::write(source.join("id.pub"), b"new public").unwrap();
        fs::write(destination.join("id.pub"), b"existing public").unwrap();

        let error = copy_key_pair(
            &source.join("id"),
            &source.join("id.pub"),
            &destination.join("id"),
            &destination.join("id.pub"),
        )
        .unwrap_err();

        assert!(error.contains("refusing to overwrite"));
        assert!(!destination.join("id").exists());
        assert_eq!(
            fs::read(destination.join("id.pub")).unwrap(),
            b"existing public"
        );
    }

    #[test]
    fn removal_deletes_both_files_and_reports_missing_pairs() {
        let temp = TestDir::new();
        let public = temp.join("id.pub");
        fs::write(temp.join("id"), b"private placeholder").unwrap();
        fs::write(&public, b"public placeholder").unwrap();
        let key = PublicKey {
            path: public.clone(),
            name: "id.pub".to_owned(),
            key_type: String::new(),
            comment: String::new(),
            fingerprint: String::new(),
            contents: String::new(),
        };

        remove_key_pair(&key).unwrap();
        assert!(!public.exists());
        assert!(!temp.join("id").exists());
        assert!(remove_key_pair(&key).is_err());
    }
}
