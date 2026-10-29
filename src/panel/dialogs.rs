use eframe::egui::{
    self, Align2, Color32, Context, CornerRadius, Frame, Margin, RichText, Stroke, TextEdit, Ui,
};
use egui_phosphor::regular::{
    CHECK_CIRCLE, FOLDER, GLOBE, HARD_DRIVES, HASH, INFO, KEY, PENCIL_SIMPLE, SPARKLE, TERMINAL,
    TRASH, USER, X_CIRCLE,
};

use crate::config;

use super::state::{Action, Panel};
use super::theme::{ERROR, MUTED, danger_button, icon_text, primary_button, secondary_button};

impl Panel {
    pub(super) fn host_form_dialog(&mut self, ctx: &Context) -> Option<Action> {
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

    pub(super) fn host_deletion_dialog(&mut self, ctx: &Context) -> Option<Action> {
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

    pub(super) fn comment_dialog(&mut self, ctx: &Context) -> Option<Action> {
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

    pub(super) fn generate_dialog(&mut self, ctx: &Context) -> Option<Action> {
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

    pub(super) fn removal_dialog(&mut self, ctx: &Context) -> Option<Action> {
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
