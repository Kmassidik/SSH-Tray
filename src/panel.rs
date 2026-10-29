use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::PathBuf;

use eframe::egui::style::WidgetVisuals;
use eframe::egui::{
    self, Align2, Color32, ComboBox, Context, CornerRadius, FontFamily, FontId, Frame, Key, Label,
    Layout, Margin, Modifiers, RichText, ScrollArea, Sense, Stroke, TextEdit, TextStyle, Ui,
};
use egui_phosphor::regular::{
    ARROW_SQUARE_OUT, ARROWS_CLOCKWISE, CHECK_CIRCLE, COPY, DESKTOP, DOWNLOAD_SIMPLE, FOLDER,
    GLOBE, HARD_DRIVES, HASH, INFO, KEY, LOCK_KEY, MAGNIFYING_GLASS, PENCIL_SIMPLE,
    PLUGS_CONNECTED, PLUS, SHIELD_CHECK, SPARKLE, TERMINAL, TERMINAL_WINDOW, TRASH, UPLOAD_SIMPLE,
    USER, X_CIRCLE,
};

use crate::{config, keys, terminal};

const APP_BG: Color32 = Color32::from_rgb(10, 11, 14);
const SIDEBAR_BG: Color32 = Color32::from_rgb(15, 17, 21);
const CARD_BG: Color32 = Color32::from_rgb(20, 22, 27);
const RAISED_BG: Color32 = Color32::from_rgb(27, 30, 36);
const BORDER: Color32 = Color32::from_rgb(43, 47, 56);
const TEXT: Color32 = Color32::from_rgb(235, 238, 244);
const MUTED: Color32 = Color32::from_rgb(145, 153, 166);
const ACCENT: Color32 = Color32::from_rgb(110, 115, 245);
const ACCENT_HOVER: Color32 = Color32::from_rgb(133, 137, 250);
const SUCCESS: Color32 = Color32::from_rgb(79, 198, 139);
const ERROR: Color32 = Color32::from_rgb(244, 113, 122);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Hosts,
    Keys,
}

struct Feedback {
    message: String,
    is_error: bool,
}

enum Action {
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

struct Panel {
    active_tab: Tab,
    hosts: Vec<config::Host>,
    public_keys: Vec<keys::PublicKey>,
    selected_host: Option<usize>,
    selected_key: Option<usize>,
    install_key: Option<usize>,
    host_search: String,
    focus_host_search: bool,
    feedback: Option<Feedback>,
    host_form_open: bool,
    host_form_original: Option<String>,
    host_draft: config::HostDraft,
    host_form_error: Option<String>,
    pending_host_deletion: Option<String>,
    host_delete_error: Option<String>,
    generate_open: bool,
    generate_name: String,
    generate_comment: String,
    generate_error: Option<String>,
    comment_edit_key: Option<PathBuf>,
    comment_edit: String,
    comment_edit_error: Option<String>,
    pending_removal: Option<PathBuf>,
}

impl Panel {
    fn new(creation: &eframe::CreationContext<'_>) -> Self {
        let mut fonts = egui::FontDefinitions::default();
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

    fn set_success(&mut self, message: impl Into<String>) {
        self.feedback = Some(Feedback {
            message: message.into(),
            is_error: false,
        });
    }

    fn set_error(&mut self, message: impl Into<String>) {
        self.feedback = Some(Feedback {
            message: message.into(),
            is_error: true,
        });
    }

    fn reload_data(&mut self) -> Result<(), String> {
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

    fn finish_with_reload(&mut self, success: String) {
        match self.reload_data() {
            Ok(()) => self.set_success(success),
            Err(error) => self.set_error(format!("{success}\nReload failed:\n{error}")),
        }
    }

    fn open_add_host(&mut self) {
        self.host_form_original = None;
        self.host_draft = empty_host_draft();
        self.host_form_error = None;
        self.host_form_open = true;
    }

    fn open_edit_host(&mut self, index: usize) {
        let Some(host) = self.hosts.get(index) else {
            self.set_error("The selected host is no longer available. Refresh and try again.");
            return;
        };
        self.host_form_original = Some(host.alias.clone());
        self.host_draft = host_draft(host);
        self.host_form_error = None;
        self.host_form_open = true;
    }

    fn top_bar(&mut self, ui: &mut Ui) -> Option<Action> {
        let mut action = None;

        ui.horizontal_wrapped(|ui| {
            brand_mark(ui);
            ui.add_space(2.0);
            ui.vertical(|ui| {
                ui.label(RichText::new("SSH Tray").size(18.0).strong().color(TEXT));
                ui.label(RichText::new("Connections & keys").small().color(MUTED));
            });
            ui.add_space(22.0);
            if tab_button(ui, self.active_tab == Tab::Hosts, HARD_DRIVES, "Hosts").clicked() {
                self.active_tab = Tab::Hosts;
            }
            if tab_button(ui, self.active_tab == Tab::Keys, KEY, "SSH Keys").clicked() {
                self.active_tab = Tab::Keys;
            }

            ui.with_layout(Layout::right_to_left(eframe::egui::Align::Center), |ui| {
                if ui
                    .add(secondary_button(icon_text(ARROWS_CLOCKWISE, "Refresh")))
                    .on_hover_text("Reload ~/.ssh/config and public keys")
                    .clicked()
                {
                    action = Some(Action::Refresh);
                }
                if self.active_tab == Tab::Keys {
                    if ui
                        .add(secondary_button(icon_text(DOWNLOAD_SIMPLE, "Import Key")))
                        .clicked()
                    {
                        action = Some(Action::ImportKey);
                    }
                    if ui
                        .add(primary_button(icon_text(PLUS, "Generate Key")))
                        .clicked()
                    {
                        self.generate_error = None;
                        self.generate_open = true;
                    }
                }
            });
        });

        if let Some(feedback) = &self.feedback {
            ui.add_space(10.0);
            let (color, icon) = if feedback.is_error {
                (ERROR, X_CIRCLE)
            } else {
                (SUCCESS, CHECK_CIRCLE)
            };
            Frame::new()
                .fill(Color32::from_rgba_unmultiplied(
                    color.r(),
                    color.g(),
                    color.b(),
                    16,
                ))
                .stroke(Stroke::new(
                    1.0_f32,
                    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 70),
                ))
                .corner_radius(CornerRadius::same(7))
                .inner_margin(Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(icon).color(color).size(16.0));
                        ui.label(RichText::new(&feedback.message).color(color));
                    });
                });
        }

        action
    }

    fn host_list(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new("HOSTS").small().strong().color(MUTED));
                ui.label(
                    RichText::new(format!("{} configured", self.hosts.len()))
                        .small()
                        .color(MUTED),
                );
            });
            ui.with_layout(Layout::right_to_left(eframe::egui::Align::Center), |ui| {
                if ui
                    .add(secondary_button(icon_text(PLUS, "Add")))
                    .on_hover_text("Add an SSH host")
                    .clicked()
                {
                    self.open_add_host();
                }
            });
        });
        ui.add_space(14.0);

        let search_response = ui.add(
            TextEdit::singleline(&mut self.host_search)
                .id_source("host_search")
                .desired_width(ui.available_width())
                .hint_text(format!("{MAGNIFYING_GLASS}  Search hosts or groups…"))
                .margin(Margin::symmetric(10, 8)),
        );
        if self.focus_host_search {
            search_response.request_focus();
            self.focus_host_search = false;
        }
        if search_response.has_focus()
            && ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape))
        {
            self.host_search.clear();
        }
        ui.add_space(4.0);
        ui.label(
            RichText::new("Cmd/Ctrl+F to focus · Esc to clear")
                .small()
                .color(MUTED),
        );
        ui.add_space(10.0);

        if self.hosts.is_empty() {
            empty_state(
                ui,
                DESKTOP,
                "No hosts configured",
                "Add a host or check ~/.ssh/config.",
            );
            return;
        }

        let query = self.host_search.trim().to_lowercase();
        let mut groups: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
        for (index, host) in self.hosts.iter().enumerate() {
            if host_matches(host, &query) {
                groups
                    .entry(host.group.as_deref().unwrap_or("Ungrouped"))
                    .or_default()
                    .push(index);
            }
        }

        if groups.is_empty() {
            empty_state(
                ui,
                MAGNIFYING_GLASS,
                "No matching hosts",
                "Try another alias or group, or clear the search.",
            );
            return;
        }

        ScrollArea::vertical()
            .id_salt("host_list")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (group, indices) in groups {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(FOLDER).color(MUTED));
                        ui.label(RichText::new(group).small().strong().color(MUTED));
                        ui.label(
                            RichText::new(indices.len().to_string())
                                .small()
                                .color(MUTED),
                        );
                    });
                    ui.add_space(5.0);
                    for index in indices {
                        let host = &self.hosts[index];
                        if host_row(ui, host, group, self.selected_host == Some(index), index)
                            .clicked()
                        {
                            self.selected_host = Some(index);
                        }
                        ui.add_space(4.0);
                    }
                    ui.add_space(12.0);
                }
            });
    }

    fn key_list(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            ui.label(RichText::new("SSH KEYS").small().strong().color(MUTED));
            ui.label(
                RichText::new(format!("{} public keys available", self.public_keys.len()))
                    .small()
                    .color(MUTED),
            );
        });
        ui.add_space(16.0);

        if self.public_keys.is_empty() {
            empty_state(
                ui,
                KEY,
                "No public keys found",
                "Generate or import a key pair to get started.",
            );
            return;
        }

        ScrollArea::vertical()
            .id_salt("key_list")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (index, key) in self.public_keys.iter().enumerate() {
                    let selected = self.selected_key == Some(index);
                    let frame = Frame::new()
                        .fill(if selected {
                            Color32::from_rgba_unmultiplied(110, 115, 245, 28)
                        } else {
                            CARD_BG
                        })
                        .stroke(Stroke::new(1.0_f32, if selected { ACCENT } else { BORDER }))
                        .corner_radius(CornerRadius::same(8))
                        .inner_margin(Margin::symmetric(11, 10))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(KEY).size(17.0).color(if selected {
                                    ACCENT_HOVER
                                } else {
                                    MUTED
                                }));
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new(&key.name).strong().color(if selected {
                                            TEXT
                                        } else {
                                            MUTED
                                        }),
                                    );
                                    ui.label(
                                        RichText::new(&key.key_type)
                                            .small()
                                            .monospace()
                                            .color(MUTED),
                                    );
                                });
                            });
                        });
                    let response = ui.interact(
                        frame.response.rect,
                        ui.id().with(("public_key", index)),
                        Sense::click(),
                    );
                    if response.clicked() {
                        self.selected_key = Some(index);
                    }
                    ui.add_space(6.0);
                }
            });
    }

    fn host_details(&mut self, ui: &mut Ui) -> Option<Action> {
        let mut action = None;
        let host = self
            .selected_host
            .and_then(|index| self.hosts.get(index))
            .cloned();

        let Some(host) = host else {
            ui.vertical_centered(|ui| {
                ui.add_space(70.0);
                empty_state(
                    ui,
                    HARD_DRIVES,
                    "Select a host",
                    "Choose an SSH alias from the sidebar, or add a new host.",
                );
                ui.add_space(14.0);
                if ui
                    .add(primary_button(icon_text(PLUS, "Add Host")))
                    .clicked()
                {
                    self.open_add_host();
                }
            });
            return None;
        };

        ui.label(RichText::new("HOST OVERVIEW").small().strong().color(MUTED));
        ui.add_space(8.0);
        card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(HARD_DRIVES).size(25.0).color(ACCENT_HOVER));
                ui.label(RichText::new(&host.alias).size(25.0).strong().color(TEXT));
                badge(ui, host.group.as_deref().unwrap_or("Ungrouped"));
            });
            if !host.aliases.is_empty() {
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!("Host {}", host.aliases.join(" ")))
                        .monospace()
                        .color(MUTED),
                );
            }
            ui.add_space(18.0);
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add(primary_button(icon_text(PLUGS_CONNECTED, "Connect")))
                    .clicked()
                {
                    action = Some(Action::Connect(host.alias.clone()));
                }
                if ui
                    .add(secondary_button(icon_text(COPY, "Copy Command")))
                    .clicked()
                {
                    action = Some(Action::CopySshCommand(host.alias.clone()));
                }
                if ui
                    .add(secondary_button(icon_text(PENCIL_SIMPLE, "Edit")))
                    .clicked()
                    && let Some(index) = self.selected_host
                {
                    self.open_edit_host(index);
                }
            });
            ui.add_space(10.0);
            ui.label(
                RichText::new(format!("{TERMINAL}  ssh {}", host.alias))
                    .monospace()
                    .color(MUTED),
            );
        });

        ui.add_space(16.0);
        section_heading(ui, GLOBE, "Connection details");
        ui.add_space(6.0);
        card(ui, |ui| {
            egui::Grid::new("host_metadata")
                .num_columns(2)
                .spacing([22.0, 14.0])
                .show(ui, |ui| {
                    for (icon, label, value) in [
                        (GLOBE, "HostName", &host.host_name),
                        (USER, "User", &host.user),
                        (HASH, "Port", &host.port),
                        (KEY, "IdentityFile", &host.identity_file),
                    ] {
                        ui.label(RichText::new(format!("{icon}  {label}")).color(MUTED));
                        if value.is_empty() {
                            ui.label(RichText::new("Not set").italics().color(MUTED));
                        } else {
                            ui.add(
                                Label::new(RichText::new(value).monospace().color(TEXT))
                                    .wrap()
                                    .selectable(true),
                            );
                        }
                        ui.end_row();
                    }
                });
        });

        ui.add_space(16.0);
        section_heading(ui, UPLOAD_SIMPLE, "Install a public key");
        ui.add_space(6.0);
        card(ui, |ui| {
            ui.label(
                RichText::new("Copy a public key to this server with ssh-copy-id in Terminal.")
                    .color(MUTED),
            );
            ui.add_space(12.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(format!("{KEY}  Public key")).color(MUTED));
                let selected_text = self
                    .install_key
                    .and_then(|key_index| self.public_keys.get(key_index))
                    .map(|key| key.name.as_str())
                    .unwrap_or("No public keys available");
                ComboBox::from_id_salt("install_public_key")
                    .selected_text(selected_text)
                    .width(280.0)
                    .show_ui(ui, |ui| {
                        for (key_index, key) in self.public_keys.iter().enumerate() {
                            ui.selectable_value(
                                &mut self.install_key,
                                Some(key_index),
                                format!("{KEY}  {}", key.name),
                            );
                        }
                    });
            });
            ui.add_space(12.0);
            let key_path = self
                .install_key
                .and_then(|key_index| self.public_keys.get(key_index))
                .map(|key| key.path.clone());
            if ui
                .add_enabled(
                    key_path.is_some(),
                    secondary_button(icon_text(ARROW_SQUARE_OUT, "Install on Server")),
                )
                .clicked()
                && let Some(public_key) = key_path
            {
                action = Some(Action::InstallKey {
                    host: host.alias.clone(),
                    public_key,
                });
            }
            ui.add_space(6.0);
            ui.label(
                RichText::new(format!(
                    "{SHIELD_CHECK}  Authentication and key passphrase prompts appear only in Terminal."
                ))
                .small()
                .color(MUTED),
            );
        });

        ui.add_space(18.0);
        danger_card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("Delete host").strong().color(TEXT));
                    ui.label(
                        RichText::new("Permanently remove this host block from ~/.ssh/config.")
                            .small()
                            .color(MUTED),
                    );
                });
                if ui
                    .add(danger_button(icon_text(TRASH, "Delete Host")))
                    .clicked()
                {
                    self.pending_host_deletion = Some(host.alias.clone());
                    self.host_delete_error = None;
                }
            });
        });

        action
    }

    fn key_details(&mut self, ui: &mut Ui) -> Option<Action> {
        let mut action = None;
        let Some(index) = self.selected_key else {
            ui.vertical_centered(|ui| {
                ui.add_space(70.0);
                empty_state(
                    ui,
                    KEY,
                    "No SSH key selected",
                    "Select a public key, or generate or import a key pair.",
                );
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if ui
                        .add(primary_button(icon_text(SPARKLE, "Generate Key")))
                        .clicked()
                    {
                        self.generate_error = None;
                        self.generate_open = true;
                    }
                    if ui
                        .add(secondary_button(icon_text(DOWNLOAD_SIMPLE, "Import Key")))
                        .clicked()
                    {
                        action = Some(Action::ImportKey);
                    }
                });
            });
            return action;
        };
        let key = self.public_keys.get(index)?.clone();

        ui.label(RichText::new("KEY DETAILS").small().strong().color(MUTED));
        ui.add_space(8.0);
        card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(KEY).size(25.0).color(ACCENT_HOVER));
                ui.label(RichText::new(&key.name).size(25.0).strong().color(TEXT));
                badge(ui, &key.key_type);
            });
            ui.add_space(18.0);
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add(primary_button(icon_text(COPY, "Copy Public Key")))
                    .clicked()
                {
                    action = Some(Action::CopyPublicKey(index));
                }
                if ui
                    .add(secondary_button(icon_text(PENCIL_SIMPLE, "Edit Comment")))
                    .clicked()
                {
                    self.comment_edit_key = Some(key.path.clone());
                    self.comment_edit = key.comment.clone();
                    self.comment_edit_error = None;
                }
                if ui
                    .add(secondary_button(icon_text(LOCK_KEY, "Change Passphrase")))
                    .clicked()
                {
                    action = Some(Action::ChangeKeyPassphrase(key.path.with_extension("")));
                }
            });
            ui.add_space(10.0);
            ui.label(
                RichText::new(format!(
                    "{SHIELD_CHECK}  Passphrase changes open securely in Terminal. No private key material or passphrase is shown here."
                ))
                .small()
                .color(MUTED),
            );
        });

        ui.add_space(16.0);
        section_heading(ui, INFO, "Public key metadata");
        ui.add_space(6.0);
        card(ui, |ui| {
            egui::Grid::new("public_key_metadata")
                .num_columns(2)
                .spacing([22.0, 14.0])
                .show(ui, |ui| {
                    ui.label(RichText::new(format!("{KEY}  Type")).color(MUTED));
                    ui.label(RichText::new(&key.key_type).monospace().color(TEXT));
                    ui.end_row();
                    ui.label(RichText::new(format!("{PENCIL_SIMPLE}  Comment")).color(MUTED));
                    if key.comment.is_empty() {
                        ui.label(RichText::new("No comment").italics().color(MUTED));
                    } else {
                        ui.label(RichText::new(&key.comment).color(TEXT));
                    }
                    ui.end_row();
                    ui.label(RichText::new(format!("{HASH}  Fingerprint")).color(MUTED));
                    ui.add(
                        Label::new(RichText::new(&key.fingerprint).monospace().color(TEXT))
                            .wrap()
                            .selectable(true),
                    );
                    ui.end_row();
                });
        });

        ui.add_space(16.0);
        section_heading(ui, TERMINAL_WINDOW, "Public key contents");
        ui.add_space(6.0);
        card(ui, |ui| {
            ui.label(
                RichText::new(
                    "Only public key material is displayed here. Select and copy it as needed.",
                )
                .small()
                .color(MUTED),
            );
            ui.add_space(10.0);
            Frame::new()
                .fill(APP_BG)
                .stroke(Stroke::new(1.0_f32, BORDER))
                .corner_radius(CornerRadius::same(7))
                .inner_margin(Margin::same(12))
                .show(ui, |ui| {
                    ScrollArea::vertical()
                        .id_salt("public_key_contents")
                        .max_height(240.0)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.add(
                                Label::new(RichText::new(&key.contents).monospace().color(TEXT))
                                    .wrap()
                                    .selectable(true),
                            );
                        });
                });
        });

        ui.add_space(18.0);
        danger_card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("Remove key pair").strong().color(TEXT));
                    ui.label(
                        RichText::new("Permanently delete both the public and private key files.")
                            .small()
                            .color(MUTED),
                    );
                });
                if ui
                    .add(danger_button(icon_text(TRASH, "Remove Key Pair")))
                    .clicked()
                {
                    action = Some(Action::RequestRemoval(key.path.clone()));
                }
            });
        });

        action
    }

    fn host_form_dialog(&mut self, ctx: &Context) -> Option<Action> {
        if !self.host_form_open {
            return None;
        }

        let editing = self.host_form_original.is_some();
        let title = if editing {
            "Edit SSH Host"
        } else {
            "Add SSH Host"
        };
        let mut open = self.host_form_open;
        let mut cancel = false;
        let mut action = None;
        egui::Window::new(format!("{PENCIL_SIMPLE}  {title}"))
            .open(&mut open)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .collapsible(false)
            .resizable(false)
            .default_width(500.0)
            .show(ctx, |ui| {
                ui.label("Manage standard SSH options here. Other options in this host block are preserved when saving.");
                ui.add_space(8.0);
                egui::Grid::new("host_editor_form")
                    .num_columns(2)
                    .spacing([12.0, 10.0])
                    .show(ui, |ui| {
                        for (label, value, hint) in [
                            ("Aliases", &mut self.host_draft.aliases, "web web-internal"),
                            ("HostName", &mut self.host_draft.host_name, "server.example.com"),
                            ("User", &mut self.host_draft.user, "Optional SSH user"),
                            ("Port", &mut self.host_draft.port, "Default: 22"),
                            ("IdentityFile", &mut self.host_draft.identity_file, "~/.ssh/id_ed25519"),
                            ("Group", &mut self.host_draft.group, "Optional panel group"),
                        ] {
                            field_label(ui, label);
                            ui.add(
                                TextEdit::singleline(value)
                                    .desired_width(320.0)
                                    .hint_text(hint),
                            );
                            ui.end_row();
                        }
                    });
                ui.small("Separate multiple aliases with spaces. All aliases share one host block.");
                if let Some(error) = &self.host_form_error {
                    ui.add_space(6.0);
                    form_error(ui, error);
                }
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.add(secondary_button("Cancel")).clicked() {
                        cancel = true;
                    }
                    let save_label = if editing { "Save Changes" } else { "Add Host" };
                    if ui.add(primary_button(icon_text(CHECK_CIRCLE, save_label))).clicked() {
                        action = Some(Action::SaveHost {
                            original_alias: self.host_form_original.clone(),
                            draft: copy_host_draft(&self.host_draft),
                        });
                    }
                });
            });

        self.host_form_open = open && !cancel && action.is_none();
        action
    }

    fn host_deletion_dialog(&mut self, ctx: &Context) -> Option<Action> {
        let alias = self.pending_host_deletion.clone()?;
        let Some(host) = self
            .hosts
            .iter()
            .find(|host| host.alias == alias || host.aliases.iter().any(|item| item == &alias))
        else {
            self.pending_host_deletion = None;
            self.host_delete_error = None;
            return None;
        };
        let aliases = if host.aliases.is_empty() {
            vec![host.alias.clone()]
        } else {
            host.aliases.clone()
        };
        let mut open = true;
        let mut cancel = false;
        let mut action = None;

        egui::Window::new(format!("{TRASH}  Delete SSH Host"))
            .open(&mut open)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .collapsible(false)
            .resizable(false)
            .default_width(460.0)
            .show(ctx, |ui| {
                ui.label("Delete this entire host block from ~/.ssh/config?");
                ui.add_space(8.0);
                ui.strong("Aliases in this block:");
                ui.label(RichText::new(aliases.join(" ")).monospace());
                ui.add_space(6.0);
                form_error(
                    ui,
                    "The block and all of its options will be permanently removed.",
                );
                if let Some(error) = &self.host_delete_error {
                    ui.add_space(6.0);
                    form_error(ui, error);
                }
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.add(secondary_button("Cancel")).clicked() {
                        cancel = true;
                    }
                    if ui
                        .add(danger_button(icon_text(TRASH, "Delete Host Block")))
                        .clicked()
                    {
                        action = Some(Action::DeleteHost(alias.clone()));
                    }
                });
            });

        self.pending_host_deletion = if open && !cancel && action.is_none() {
            Some(alias)
        } else {
            None
        };
        action
    }

    fn comment_dialog(&mut self, ctx: &Context) -> Option<Action> {
        let path = self.comment_edit_key.clone()?;
        let Some(key) = self.public_keys.iter().find(|key| key.path == path) else {
            self.comment_edit_key = None;
            self.comment_edit_error = None;
            return None;
        };
        let name = key.name.clone();
        let mut open = true;
        let mut cancel = false;
        let mut action = None;

        egui::Window::new(format!("{PENCIL_SIMPLE}  Edit Public Key Comment"))
            .open(&mut open)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .collapsible(false)
            .resizable(false)
            .default_width(460.0)
            .show(ctx, |ui| {
                ui.label(format!("Update the comment stored in {name}."));
                ui.add_space(8.0);
                field_label(ui, "Comment");
                ui.add(
                    TextEdit::singleline(&mut self.comment_edit)
                        .desired_width(400.0)
                        .hint_text("Optional public key comment"),
                );
                ui.small("Only the public key comment changes; the key material and private key are untouched.");
                if let Some(error) = &self.comment_edit_error {
                    ui.add_space(6.0);
                    form_error(ui, error);
                }
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.add(secondary_button("Cancel")).clicked() {
                        cancel = true;
                    }
                    if ui.add(primary_button(icon_text(CHECK_CIRCLE, "Save Comment"))).clicked() {
                        action = Some(Action::UpdatePublicKeyComment {
                            path: path.clone(),
                            comment: self.comment_edit.clone(),
                        });
                    }
                });
            });

        self.comment_edit_key = if open && !cancel && action.is_none() {
            Some(path)
        } else {
            None
        };
        action
    }

    fn generate_dialog(&mut self, ctx: &Context) -> Option<Action> {
        if !self.generate_open {
            return None;
        }

        let mut open = self.generate_open;
        let mut cancel = false;
        let mut action = None;
        egui::Window::new(format!("{SPARKLE}  Generate SSH Key"))
            .open(&mut open)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .collapsible(false)
            .resizable(false)
            .default_width(460.0)
            .show(ctx, |ui| {
                ui.label("Create a new SSH key pair in ~/.ssh using ssh-keygen.");
                ui.add_space(8.0);
                egui::Grid::new("generate_key_form")
                    .num_columns(2)
                    .spacing([12.0, 10.0])
                    .show(ui, |ui| {
                        field_label(ui, "New filename");
                        ui.add(
                            TextEdit::singleline(&mut self.generate_name)
                                .desired_width(280.0)
                                .hint_text("id_ed25519_work"),
                        );
                        ui.end_row();
                        field_label(ui, "Comment");
                        ui.add(
                            TextEdit::singleline(&mut self.generate_comment)
                                .desired_width(280.0)
                                .hint_text("Optional key comment"),
                        );
                        ui.end_row();
                    });
                ui.add_space(8.0);
                ui.label(
                    RichText::new(
                        "Terminal will ask whether to protect the private key with a passphrase. Enter it only in Terminal, never in this window.",
                    )
                    .weak(),
                );
                if let Some(error) = &self.generate_error {
                    ui.add_space(6.0);
                    form_error(ui, error);
                }
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.add(secondary_button("Cancel")).clicked() {
                        cancel = true;
                    }
                    if ui.add(primary_button(icon_text(TERMINAL, "Generate in Terminal"))).clicked() {
                        action = Some(Action::GenerateKey {
                            file_name: self.generate_name.clone(),
                            comment: self.generate_comment.clone(),
                        });
                    }
                });
            });

        self.generate_open = open && !cancel && action.is_none();
        action
    }

    fn removal_dialog(&mut self, ctx: &Context) -> Option<Action> {
        let path = self.pending_removal.clone()?;
        let Some(key) = self.public_keys.iter().find(|key| key.path == path) else {
            self.pending_removal = None;
            return None;
        };
        let name = key.name.clone();
        let mut open = true;
        let mut cancel = false;
        let mut action = None;

        egui::Window::new(format!("{TRASH}  Remove SSH Key Pair"))
            .open(&mut open)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .collapsible(false)
            .resizable(false)
            .default_width(430.0)
            .show(ctx, |ui| {
                ui.label(format!("Remove {name} and its private key?"));
                ui.add_space(6.0);
                form_error(
                    ui,
                    "This permanently deletes both key files. This cannot be undone.",
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.add(secondary_button("Cancel")).clicked() {
                        cancel = true;
                    }
                    if ui
                        .add(danger_button(icon_text(TRASH, "Remove Key Pair")))
                        .clicked()
                    {
                        action = Some(Action::RemoveKey(path.clone()));
                    }
                });
            });

        self.pending_removal = if open && !cancel && action.is_none() {
            Some(path)
        } else {
            None
        };
        action
    }

    fn perform(&mut self, action: Action, ctx: &Context) {
        match action {
            Action::Refresh => match self.reload_data() {
                Ok(()) => self.set_success(format!(
                    "Refreshed {} hosts and {} public keys.",
                    self.hosts.len(),
                    self.public_keys.len()
                )),
                Err(error) => self.set_error(error),
            },
            Action::SaveHost {
                original_alias,
                draft,
            } => {
                let editing = original_alias.is_some();
                let selected_alias = draft
                    .aliases
                    .split_whitespace()
                    .next()
                    .map(str::to_owned);
                let display_name = selected_alias.clone().unwrap_or_else(|| "host".to_owned());
                match config::save_host(original_alias.as_deref(), &draft) {
                    Ok(()) => {
                        self.host_form_open = false;
                        self.host_form_original = None;
                        self.host_form_error = None;
                        self.host_draft = empty_host_draft();
                        self.finish_with_reload(format!(
                            "{} {display_name}.",
                            if editing { "Updated" } else { "Added" }
                        ));
                        if let Some(alias) = selected_alias {
                            self.selected_host = self
                                .hosts
                                .iter()
                                .position(|host| {
                                    host.alias == alias
                                        || host.aliases.iter().any(|item| item == &alias)
                                })
                                .or(self.selected_host);
                        }
                    }
                    Err(error) => {
                        self.host_form_error = Some(error.clone());
                        self.host_form_open = true;
                        self.set_error(error);
                    }
                }
            }
            Action::DeleteHost(alias) => {
                let aliases = self
                    .hosts
                    .iter()
                    .find(|host| {
                        host.alias == alias || host.aliases.iter().any(|item| item == &alias)
                    })
                    .map(|host| {
                        if host.aliases.is_empty() {
                            vec![host.alias.clone()]
                        } else {
                            host.aliases.clone()
                        }
                    });
                let Some(aliases) = aliases else {
                    self.pending_host_deletion = None;
                    self.host_delete_error = None;
                    self.set_error(
                        "The selected host is no longer available. Refresh and try again.",
                    );
                    return;
                };
                match config::delete_host(&alias) {
                    Ok(()) => {
                        self.pending_host_deletion = None;
                        self.host_delete_error = None;
                        self.finish_with_reload(format!(
                            "Deleted host block {}.",
                            aliases.join(" ")
                        ));
                    }
                    Err(error) => {
                        self.host_delete_error = Some(error.clone());
                        self.pending_host_deletion = Some(alias);
                        self.set_error(error);
                    }
                }
            }
            Action::Connect(host) => match terminal::open_ssh(&host) {
                Ok(()) => self.set_success(format!("Opening Terminal to connect to {host}.")),
                Err(error) => self.set_error(error),
            },
            Action::CopySshCommand(host) => {
                ctx.copy_text(format!("ssh {host}"));
                self.set_success(format!("Copied SSH command for {host}."));
            }
            Action::InstallKey { host, public_key } => {
                let key_name = public_key
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "public key".to_owned());
                match terminal::open_copy_id(&host, &public_key) {
                    Ok(()) => self
                        .set_success(format!("Opening Terminal to install {key_name} on {host}.")),
                    Err(error) => self.set_error(error),
                }
            }
            Action::CopyPublicKey(index) => match self.public_keys.get(index) {
                Some(key) => {
                    ctx.copy_text(key.contents.clone());
                    self.set_success(format!("Copied public key {}.", key.name));
                }
                None => self.set_error(
                    "The selected public key is no longer available. Refresh and try again.",
                ),
            },
            Action::GenerateKey { file_name, comment } => {
                match validated_new_key(&file_name, &comment)
                    .and_then(|(path, comment)| terminal::open_generate_key(&path, &comment))
                {
                    Ok(()) => {
                        self.generate_open = false;
                        self.generate_error = None;
                        self.generate_name.clear();
                        self.generate_comment.clear();
                        self.finish_with_reload(format!(
                            "Opened Terminal to generate {file_name}. Complete the prompts there, then use Refresh when generation finishes."
                        ));
                    }
                    Err(error) => {
                        self.generate_error = Some(error.clone());
                        self.generate_open = true;
                        self.set_error(error);
                    }
                }
            }
            Action::ImportKey => {
                let source = rfd::FileDialog::new()
                    .set_title("Select a private SSH key to import")
                    .pick_file();
                if let Some(source) = source {
                    match keys::import_key_pair(&source) {
                        Ok(imported) => {
                            let imported_path = imported.path.clone();
                            let imported_name = imported.name.clone();
                            self.finish_with_reload(format!(
                                "Imported {imported_name} and its public key."
                            ));
                            self.selected_key = self
                                .public_keys
                                .iter()
                                .position(|key| key.path == imported_path);
                            self.install_key = self.selected_key;
                        }
                        Err(error) => self.set_error(error),
                    }
                }
            }
            Action::UpdatePublicKeyComment { path, comment } => {
                let key = self
                    .public_keys
                    .iter()
                    .find(|key| key.path == path)
                    .cloned();
                match key {
                    Some(key) => match keys::update_public_key_comment(&key, &comment) {
                        Ok(updated) => {
                            let updated_path = updated.path.clone();
                            let updated_name = updated.name.clone();
                            self.comment_edit_key = None;
                            self.comment_edit_error = None;
                            self.comment_edit.clear();
                            self.finish_with_reload(format!(
                                "Updated public key comment for {updated_name}."
                            ));
                            self.selected_key = self
                                .public_keys
                                .iter()
                                .position(|key| key.path == updated_path)
                                .or(self.selected_key);
                        }
                        Err(error) => {
                            self.comment_edit_error = Some(error.clone());
                            self.comment_edit_key = Some(path);
                            self.set_error(error);
                        }
                    },
                    None => {
                        self.comment_edit_key = None;
                        self.comment_edit_error = None;
                        self.set_error(
                            "The selected key is no longer available. Refresh and try again.",
                        );
                    }
                }
            }
            Action::ChangeKeyPassphrase(private_key_path) => {
                match terminal::open_change_key_passphrase(&private_key_path) {
                    Ok(()) => self.set_success(format!(
                        "Opened Terminal to change the passphrase for {}. Complete the secure prompts there.",
                        private_key_path.display()
                    )),
                    Err(error) => self.set_error(error),
                }
            }
            Action::RequestRemoval(path) => self.pending_removal = Some(path),
            Action::RemoveKey(path) => {
                self.pending_removal = None;
                let selected = self.public_keys.iter().find(|key| key.path == path);
                match selected {
                    Some(key) => {
                        let name = key.name.clone();
                        match keys::remove_key_pair(key) {
                            Ok(()) => self
                                .finish_with_reload(format!("Removed {name} and its private key.")),
                            Err(error) => self.set_error(error),
                        }
                    }
                    None => self.set_error(
                        "The selected key is no longer available. Refresh and try again.",
                    ),
                }
            }
        }
    }
}

impl eframe::App for Panel {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        if ctx.input_mut(|input| input.consume_key(Modifiers::COMMAND, Key::F)) {
            self.active_tab = Tab::Hosts;
            self.focus_host_search = true;
        }

        let mut action = egui::TopBottomPanel::top("panel_header")
            .frame(
                Frame::new()
                    .fill(SIDEBAR_BG)
                    .stroke(Stroke::new(1.0_f32, BORDER))
                    .inner_margin(Margin::symmetric(18, 12)),
            )
            .show(ctx, |ui| self.top_bar(ui))
            .inner;

        egui::SidePanel::left("panel_navigation")
            .frame(
                Frame::new()
                    .fill(SIDEBAR_BG)
                    .stroke(Stroke::new(1.0_f32, BORDER))
                    .inner_margin(Margin::symmetric(14, 18)),
            )
            .default_width(270.0)
            .min_width(220.0)
            .max_width(390.0)
            .resizable(true)
            .show(ctx, |ui| match self.active_tab {
                Tab::Hosts => self.host_list(ui),
                Tab::Keys => self.key_list(ui),
            });

        let detail_action = egui::CentralPanel::default()
            .frame(
                Frame::new()
                    .fill(APP_BG)
                    .inner_margin(Margin::symmetric(24, 22)),
            )
            .show(ctx, |ui| {
                ScrollArea::vertical()
                    .id_salt("detail_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| match self.active_tab {
                        Tab::Hosts => self.host_details(ui),
                        Tab::Keys => self.key_details(ui),
                    })
                    .inner
            })
            .inner;
        if detail_action.is_some() {
            action = detail_action;
        }

        if let Some(dialog_action) = self.host_form_dialog(ctx) {
            action = Some(dialog_action);
        }
        if let Some(dialog_action) = self.host_deletion_dialog(ctx) {
            action = Some(dialog_action);
        }
        if let Some(dialog_action) = self.comment_dialog(ctx) {
            action = Some(dialog_action);
        }
        if let Some(dialog_action) = self.generate_dialog(ctx) {
            action = Some(dialog_action);
        }
        if let Some(dialog_action) = self.removal_dialog(ctx) {
            action = Some(dialog_action);
        }
        if let Some(action) = action {
            self.perform(action, ctx);
        }
    }
}

fn configure_theme(ctx: &Context) {
    let mut style = (*ctx.style()).clone();
    style.text_styles.insert(
        TextStyle::Heading,
        FontId::new(25.0, FontFamily::Proportional),
    );
    style
        .text_styles
        .insert(TextStyle::Body, FontId::new(14.0, FontFamily::Proportional));
    style.text_styles.insert(
        TextStyle::Button,
        FontId::new(13.5, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Small,
        FontId::new(11.5, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Monospace,
        FontId::new(12.5, FontFamily::Monospace),
    );
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(11.0_f32, 7.0);
    style.spacing.window_margin = Margin::same(18);
    style.spacing.menu_margin = Margin::same(8);
    style.spacing.interact_size = egui::vec2(40.0, 32.0);
    style.spacing.scroll = egui::style::ScrollStyle::thin();

    style.visuals = egui::Visuals::dark();
    style.visuals.panel_fill = APP_BG;
    style.visuals.window_fill = CARD_BG;
    style.visuals.window_stroke = Stroke::new(1.0_f32, BORDER);
    style.visuals.window_corner_radius = CornerRadius::same(11);
    style.visuals.menu_corner_radius = CornerRadius::same(8);
    style.visuals.extreme_bg_color = APP_BG;
    style.visuals.faint_bg_color = SIDEBAR_BG;
    style.visuals.code_bg_color = APP_BG;
    style.visuals.hyperlink_color = ACCENT_HOVER;
    style.visuals.warn_fg_color = Color32::from_rgb(235, 181, 95);
    style.visuals.error_fg_color = ERROR;
    style.visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(110, 115, 245, 85);
    style.visuals.selection.stroke = Stroke::new(1.0_f32, ACCENT_HOVER);
    style.visuals.widgets.noninteractive = WidgetVisuals {
        bg_fill: CARD_BG,
        weak_bg_fill: CARD_BG,
        bg_stroke: Stroke::new(1.0_f32, BORDER),
        corner_radius: CornerRadius::same(7),
        fg_stroke: Stroke::new(1.0_f32, TEXT),
        expansion: 0.0,
    };
    style.visuals.widgets.inactive = WidgetVisuals {
        bg_fill: RAISED_BG,
        weak_bg_fill: RAISED_BG,
        bg_stroke: Stroke::new(1.0_f32, BORDER),
        corner_radius: CornerRadius::same(7),
        fg_stroke: Stroke::new(1.0_f32, TEXT),
        expansion: 0.0,
    };
    style.visuals.widgets.hovered = WidgetVisuals {
        bg_fill: Color32::from_rgb(34, 38, 47),
        weak_bg_fill: Color32::from_rgb(34, 38, 47),
        bg_stroke: Stroke::new(1.0_f32, ACCENT),
        corner_radius: CornerRadius::same(7),
        fg_stroke: Stroke::new(1.0_f32, Color32::WHITE),
        expansion: 0.0,
    };
    style.visuals.widgets.active = WidgetVisuals {
        bg_fill: ACCENT,
        weak_bg_fill: ACCENT,
        bg_stroke: Stroke::new(1.0_f32, ACCENT_HOVER),
        corner_radius: CornerRadius::same(7),
        fg_stroke: Stroke::new(1.0_f32, Color32::WHITE),
        expansion: 0.0,
    };
    style.visuals.widgets.open = style.visuals.widgets.hovered;
    ctx.set_style(style);
}

fn icon_text(icon: &str, text: &str) -> String {
    format!("{icon}  {text}")
}

fn primary_button(text: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text.into()).strong().color(Color32::WHITE))
        .fill(ACCENT)
        .stroke(Stroke::new(1.0_f32, ACCENT_HOVER))
        .corner_radius(CornerRadius::same(7))
        .min_size(egui::vec2(0.0, 34.0))
}

fn secondary_button(text: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text.into()).color(TEXT))
        .fill(RAISED_BG)
        .stroke(Stroke::new(1.0_f32, BORDER))
        .corner_radius(CornerRadius::same(7))
        .min_size(egui::vec2(0.0, 34.0))
}

fn danger_button(text: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text.into()).strong().color(ERROR))
        .fill(Color32::from_rgba_unmultiplied(244, 113, 122, 18))
        .stroke(Stroke::new(
            1.0_f32,
            Color32::from_rgba_unmultiplied(244, 113, 122, 95),
        ))
        .corner_radius(CornerRadius::same(7))
        .min_size(egui::vec2(0.0, 34.0))
}

fn tab_button(ui: &mut Ui, selected: bool, icon: &str, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(
            RichText::new(icon_text(icon, text))
                .strong()
                .color(if selected { TEXT } else { MUTED }),
        )
        .fill(if selected {
            Color32::from_rgba_unmultiplied(110, 115, 245, 32)
        } else {
            Color32::TRANSPARENT
        })
        .stroke(if selected {
            Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(110, 115, 245, 110))
        } else {
            Stroke::NONE
        })
        .corner_radius(CornerRadius::same(7))
        .min_size(egui::vec2(0.0, 36.0)),
    )
}

fn brand_mark(ui: &mut Ui) {
    Frame::new()
        .fill(ACCENT)
        .corner_radius(CornerRadius::same(9))
        .inner_margin(Margin::same(10))
        .show(ui, |ui| {
            ui.label(
                RichText::new(TERMINAL_WINDOW)
                    .size(19.0)
                    .color(Color32::WHITE),
            );
        });
}

fn badge(ui: &mut Ui, text: &str) {
    Frame::new()
        .fill(RAISED_BG)
        .stroke(Stroke::new(1.0_f32, BORDER))
        .corner_radius(CornerRadius::same(5))
        .inner_margin(Margin::symmetric(7, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(text).small().strong().color(MUTED));
        });
}

fn card(ui: &mut Ui, contents: impl FnOnce(&mut Ui)) {
    Frame::new()
        .fill(CARD_BG)
        .stroke(Stroke::new(1.0_f32, BORDER))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::same(18))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            contents(ui);
        });
}

fn danger_card(ui: &mut Ui, contents: impl FnOnce(&mut Ui)) {
    Frame::new()
        .fill(Color32::from_rgba_unmultiplied(244, 113, 122, 8))
        .stroke(Stroke::new(
            1.0_f32,
            Color32::from_rgba_unmultiplied(244, 113, 122, 55),
        ))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::same(16))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            contents(ui);
        });
}

fn section_heading(ui: &mut Ui, icon: &str, text: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(icon).size(16.0).color(ACCENT_HOVER));
        ui.label(RichText::new(text).size(16.0).strong().color(TEXT));
    });
}

fn empty_state(ui: &mut Ui, icon: &str, title: &str, description: &str) {
    ui.vertical_centered(|ui| {
        ui.add_space(12.0);
        ui.label(RichText::new(icon).size(32.0).color(MUTED));
        ui.add_space(6.0);
        ui.label(RichText::new(title).size(18.0).strong().color(TEXT));
        ui.label(RichText::new(description).color(MUTED));
        ui.add_space(12.0);
    });
}

fn host_matches(host: &config::Host, query: &str) -> bool {
    query.is_empty()
        || host.alias.to_lowercase().contains(query)
        || host
            .aliases
            .iter()
            .any(|alias| alias.to_lowercase().contains(query))
        || host
            .group
            .as_deref()
            .is_some_and(|group| group.to_lowercase().contains(query))
}

fn host_row(
    ui: &mut Ui,
    host: &config::Host,
    group: &str,
    selected: bool,
    index: usize,
) -> egui::Response {
    let frame = Frame::new()
        .fill(if selected {
            Color32::from_rgba_unmultiplied(110, 115, 245, 28)
        } else {
            CARD_BG
        })
        .stroke(Stroke::new(1.0_f32, if selected { ACCENT } else { BORDER }))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(10, 9))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(HARD_DRIVES).size(17.0).color(if selected {
                    ACCENT_HOVER
                } else {
                    MUTED
                }));
                ui.vertical(|ui| {
                    ui.label(RichText::new(&host.alias).strong().color(if selected {
                        TEXT
                    } else {
                        MUTED
                    }));
                    ui.label(
                        RichText::new(if host.host_name.is_empty() {
                            "No hostname set"
                        } else {
                            &host.host_name
                        })
                        .small()
                        .color(MUTED),
                    );
                });
                ui.with_layout(Layout::right_to_left(eframe::egui::Align::Center), |ui| {
                    badge(ui, group);
                });
            });
        });
    ui.interact(
        frame.response.rect,
        ui.id().with(("host", index)),
        Sense::click(),
    )
}

fn field_label(ui: &mut Ui, label: &str) {
    let icon = match label {
        "Aliases" => HARD_DRIVES,
        "HostName" => GLOBE,
        "User" => USER,
        "Port" => HASH,
        "IdentityFile" | "New filename" => KEY,
        "Group" => FOLDER,
        "Comment" => PENCIL_SIMPLE,
        _ => INFO,
    };
    ui.label(
        RichText::new(format!("{icon}  {label}"))
            .strong()
            .color(MUTED),
    );
}

fn form_error(ui: &mut Ui, message: &str) {
    Frame::new()
        .fill(Color32::from_rgba_unmultiplied(244, 113, 122, 12))
        .stroke(Stroke::new(
            1.0_f32,
            Color32::from_rgba_unmultiplied(244, 113, 122, 55),
        ))
        .corner_radius(CornerRadius::same(7))
        .inner_margin(Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(X_CIRCLE).color(ERROR));
                ui.label(RichText::new(message).color(ERROR));
            });
        });
}

pub fn run() -> Result<(), String> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("SSH Tray")
            .with_inner_size([960.0, 640.0])
            .with_min_inner_size([720.0, 480.0])
            .with_resizable(true),
        ..Default::default()
    };

    eframe::run_native(
        "SSH Tray",
        options,
        Box::new(|creation| Ok(Box::new(Panel::new(creation)))),
    )
    .map_err(|error| format!("failed to run SSH Tray panel: {error}"))
}

fn empty_host_draft() -> config::HostDraft {
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

fn copy_host_draft(draft: &config::HostDraft) -> config::HostDraft {
    config::HostDraft {
        aliases: draft.aliases.clone(),
        host_name: draft.host_name.clone(),
        user: draft.user.clone(),
        port: draft.port.clone(),
        identity_file: draft.identity_file.clone(),
        group: draft.group.clone(),
    }
}

fn clamped_selection(previous: Option<usize>, length: usize) -> Option<usize> {
    if length == 0 {
        None
    } else {
        Some(previous.unwrap_or(0).min(length - 1))
    }
}

fn validated_new_key(file_name: &str, comment: &str) -> Result<(PathBuf, String), String> {
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
