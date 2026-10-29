use std::ffi::OsString;
use std::path::PathBuf;

use crate::{config, keys};

use super::theme::configure_theme;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Tab {
    Hosts,
    Keys,
}

pub(super) struct Feedback {
    pub(super) message: String,
    pub(super) is_error: bool,
}

pub(super) enum Action {
    Refresh,
    Connect(String),
    CopySshCommand(String),
    SaveHost {
        original_alias: Option<String>,
        draft: config::HostDraft,
    },
    DeleteHost(String),
    InstallKey {
        host: String,
        public_key: PathBuf,
    },
    CopyPublicKey(usize),
    GenerateKey {
        file_name: String,
        comment: String,
    },
    ImportKey,
    UpdatePublicKeyComment {
        path: PathBuf,
        comment: String,
    },
    ChangeKeyPassphrase(PathBuf),
    RequestRemoval(PathBuf),
    RemoveKey(PathBuf),
}

pub(super) struct Panel {
    pub(super) active_tab: Tab,
    pub(super) hosts: Vec<config::Host>,
    pub(super) public_keys: Vec<keys::PublicKey>,
    pub(super) selected_host: Option<usize>,
    pub(super) selected_key: Option<usize>,
    pub(super) install_key: Option<usize>,
    pub(super) host_search: String,
    pub(super) focus_host_search: bool,
    pub(super) feedback: Option<Feedback>,
    pub(super) host_form_open: bool,
    pub(super) host_form_original: Option<String>,
    pub(super) host_draft: config::HostDraft,
    pub(super) host_form_error: Option<String>,
    pub(super) pending_host_deletion: Option<String>,
    pub(super) host_delete_error: Option<String>,
    pub(super) generate_open: bool,
    pub(super) generate_name: String,
    pub(super) generate_comment: String,
    pub(super) generate_error: Option<String>,
    pub(super) comment_edit_key: Option<PathBuf>,
    pub(super) comment_edit: String,
    pub(super) comment_edit_error: Option<String>,
    pub(super) pending_removal: Option<PathBuf>,
}

impl Panel {
    pub(super) fn new(creation: &eframe::CreationContext<'_>) -> Self {
        let mut fonts = eframe::egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        creation.egui_ctx.set_fonts(fonts);
        configure_theme(&creation.egui_ctx);

        let mut panel = Self {
            active_tab: Tab::Hosts,
            hosts: Vec::new(),
            public_keys: Vec::new(),
            selected_host: None,
            selected_key: None,
            install_key: None,
            host_search: String::new(),
            focus_host_search: false,
            feedback: None,
            host_form_open: false,
            host_form_original: None,
            host_draft: empty_host_draft(),
            host_form_error: None,
            pending_host_deletion: None,
            host_delete_error: None,
            generate_open: false,
            generate_name: String::new(),
            generate_comment: String::new(),
            generate_error: None,
            comment_edit_key: None,
            comment_edit: String::new(),
            comment_edit_error: None,
            pending_removal: None,
        };

        if let Err(error) = panel.reload_data() {
            panel.set_error(error);
        }
        panel
    }

    pub(super) fn set_success(&mut self, message: impl Into<String>) {
        self.feedback = Some(Feedback {
            message: message.into(),
            is_error: false,
        });
    }

    pub(super) fn set_error(&mut self, message: impl Into<String>) {
        self.feedback = Some(Feedback {
            message: message.into(),
            is_error: true,
        });
    }

    pub(super) fn reload_data(&mut self) -> Result<(), String> {
        let old_host_index = self.selected_host;
        let old_host = self
            .selected_host
            .and_then(|index| self.hosts.get(index))
            .map(|host| host.alias.clone());
        let old_key_index = self.selected_key;
        let old_key = self
            .selected_key
            .and_then(|index| self.public_keys.get(index))
            .map(|key| key.path.clone());
        let old_install_index = self.install_key;
        let old_install_key = self
            .install_key
            .and_then(|index| self.public_keys.get(index))
            .map(|key| key.path.clone());

        let hosts = config::load_hosts();
        let public_keys = keys::load_public_keys();
        let mut errors = Vec::new();

        match hosts {
            Ok(hosts) => self.hosts = hosts,
            Err(error) => {
                self.hosts.clear();
                errors.push(format!("Could not load SSH hosts: {error}"));
            }
        }
        match public_keys {
            Ok(public_keys) => self.public_keys = public_keys,
            Err(error) => {
                self.public_keys.clear();
                errors.push(format!("Could not load SSH keys: {error}"));
            }
        }

        self.selected_host = old_host
            .as_ref()
            .and_then(|alias| self.hosts.iter().position(|host| &host.alias == alias))
            .or_else(|| clamped_selection(old_host_index, self.hosts.len()));
        self.selected_key = old_key
            .as_ref()
            .and_then(|path| self.public_keys.iter().position(|key| &key.path == path))
            .or_else(|| clamped_selection(old_key_index, self.public_keys.len()));
        self.install_key = old_install_key
            .as_ref()
            .and_then(|path| self.public_keys.iter().position(|key| &key.path == path))
            .or_else(|| clamped_selection(old_install_index, self.public_keys.len()));

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("\n"))
        }
    }

    pub(super) fn finish_with_reload(&mut self, success: String) {
        match self.reload_data() {
            Ok(()) => self.set_success(success),
            Err(error) => self.set_error(format!("{success}\nReload failed:\n{error}")),
        }
    }

    pub(super) fn open_add_host(&mut self) {
        self.host_form_original = None;
        self.host_draft = empty_host_draft();
        self.host_form_error = None;
        self.host_form_open = true;
    }

    pub(super) fn open_edit_host(&mut self, index: usize) {
        let Some(host) = self.hosts.get(index) else {
            self.set_error("The selected host is no longer available. Refresh and try again.");
            return;
        };
        self.host_form_original = Some(host.alias.clone());
        self.host_draft = host_draft(host);
        self.host_form_error = None;
        self.host_form_open = true;
    }
}

pub(super) fn empty_host_draft() -> config::HostDraft {
    config::HostDraft {
        aliases: String::new(),
        host_name: String::new(),
        user: String::new(),
        port: String::new(),
        identity_file: String::new(),
        group: String::new(),
    }
}

fn host_draft(host: &config::Host) -> config::HostDraft {
    config::HostDraft {
        aliases: if host.aliases.is_empty() {
            host.alias.clone()
        } else {
            host.aliases.join(" ")
        },
        host_name: host.host_name.clone(),
        user: host.user.clone(),
        port: host.port.clone(),
        identity_file: host.identity_file.clone(),
        group: host.group.clone().unwrap_or_default(),
    }
}

fn clamped_selection(previous: Option<usize>, length: usize) -> Option<usize> {
    if length == 0 {
        None
    } else {
        Some(previous.unwrap_or(0).min(length - 1))
    }
}

pub(super) fn validated_new_key(
    file_name: &str,
    comment: &str,
) -> Result<(PathBuf, String), String> {
    let file_name = file_name.trim();
    if file_name.is_empty() {
        return Err("Enter a filename for the new key.".to_owned());
    }
    if matches!(file_name, "." | "..")
        || !file_name.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        })
    {
        return Err(
            "The filename may contain only letters, numbers, dots, underscores, and hyphens."
                .to_owned(),
        );
    }
    if file_name.ends_with(".pub") {
        return Err("Enter the private key filename without the .pub suffix.".to_owned());
    }
    if comment.contains(['\n', '\r']) {
        return Err("The key comment must be a single line.".to_owned());
    }

    let path = keys::key_path(file_name)?;
    if path.exists() {
        return Err(format!(
            "~/.ssh/{file_name} already exists. Choose another filename."
        ));
    }
    let mut public_name = OsString::from(path.as_os_str());
    public_name.push(".pub");
    if PathBuf::from(public_name).exists() {
        return Err(format!(
            "~/.ssh/{file_name}.pub already exists. Choose another filename."
        ));
    }

    Ok((path, comment.trim().to_owned()))
}
