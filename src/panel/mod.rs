mod dialogs;
mod hosts;
mod keys;
mod state;
mod theme;

use eframe::egui::{
    self, Color32, Context, CornerRadius, Frame, Key, Margin, Modifiers, RichText, ScrollArea,
    Stroke, Ui,
};
use egui_phosphor::regular::{
    ARROWS_CLOCKWISE, CHECK_CIRCLE, DOWNLOAD_SIMPLE, HARD_DRIVES, KEY, PLUS, X_CIRCLE,
};

use crate::{config, keys as key_store, terminal};

use self::state::{Action, Panel, Tab, empty_host_draft, validated_new_key};
use self::theme::{
    APP_BG, BORDER, ERROR, MUTED, SIDEBAR_BG, SUCCESS, TEXT, brand_mark, icon_text, primary_button,
    secondary_button, tab_button,
};

impl Panel {
    fn navigation(&mut self, ui: &mut Ui) -> Option<Action> {
        let mut action = None;

        ui.horizontal(|ui| {
            brand_mark(ui);
            ui.add_space(2.0);
            ui.vertical(|ui| {
                ui.label(RichText::new("SSH Tray").size(18.0).strong().color(TEXT));
                ui.label(RichText::new("Connections & keys").small().color(MUTED));
            });
        });
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            if tab_button(ui, self.active_tab == Tab::Hosts, HARD_DRIVES, "Hosts").clicked() {
                self.active_tab = Tab::Hosts;
            }
            if tab_button(ui, self.active_tab == Tab::Keys, KEY, "SSH Keys").clicked() {
                self.active_tab = Tab::Keys;
            }
        });
        ui.add_space(10.0);
        if ui
            .add_sized(
                [ui.available_width(), 34.0],
                secondary_button(icon_text(ARROWS_CLOCKWISE, "Refresh")),
            )
            .on_hover_text("Reload ~/.ssh/config and public keys")
            .clicked()
        {
            action = Some(Action::Refresh);
        }
        if self.active_tab == Tab::Keys {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui
                    .add(primary_button(icon_text(PLUS, "Generate Key")))
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
        }

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

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(12.0);
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
                    match key_store::import_key_pair(&source) {
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
                    Some(key) => match key_store::update_public_key_comment(&key, &comment) {
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
                        match key_store::remove_key_pair(key) {
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

        let mut action = egui::SidePanel::left("panel_navigation")
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
            .show(ctx, |ui| {
                let action = self.navigation(ui);
                match self.active_tab {
                    Tab::Hosts => self.host_list(ui),
                    Tab::Keys => self.key_list(ui),
                }
                action
            })
            .inner;

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
