use std::env;
use std::path::{Path, PathBuf};

mod files;
mod inventory;

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
    inventory::load_public_keys_in(&ssh_dir()?)
}

pub fn key_path(file_name: &str) -> Result<PathBuf, String> {
    files::key_path_in(&ssh_dir()?, file_name)
}

pub fn import_key_pair(source: &Path) -> Result<PublicKey, String> {
    files::import_key_pair_in(&ssh_dir()?, source)
}

pub fn remove_key_pair(key: &PublicKey) -> Result<(), String> {
    files::remove_pair_files(&key.path)
}

/// Replace only the comment in a public key file and return refreshed metadata.
pub fn update_public_key_comment(key: &PublicKey, comment: &str) -> Result<PublicKey, String> {
    inventory::update_public_key_comment(key, comment)
}

fn ssh_dir() -> Result<PathBuf, String> {
    let home =
        env::var_os("HOME").ok_or_else(|| "HOME is not set; cannot locate ~/.ssh".to_owned())?;
    Ok(PathBuf::from(home).join(".ssh"))
}

#[cfg(test)]
mod tests {
    use super::files::{copy_key_pair, key_path_in};
    use super::inventory::{parse_public_key, replace_public_key_comment};
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
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
