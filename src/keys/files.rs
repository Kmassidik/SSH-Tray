use super::{PublicKey, inventory::read_public_key};
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

pub(super) fn key_path_in(dir: &Path, file_name: &str) -> Result<PathBuf, String> {
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

pub(super) fn import_key_pair_in(dir: &Path, source: &Path) -> Result<PublicKey, String> {
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

pub(super) fn copy_key_pair(
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

pub(super) fn remove_pair_files(public_path: &Path) -> Result<(), String> {
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
