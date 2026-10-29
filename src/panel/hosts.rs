use std::collections::BTreeMap;

use eframe::egui::{
    self, Color32, ComboBox, CornerRadius, Frame, Key, Label, Layout, Margin, Modifiers, RichText,
    ScrollArea, Sense, Stroke, TextEdit, Ui,
};
use egui_phosphor::regular::{
    ARROW_SQUARE_OUT, COPY, DESKTOP, FOLDER, GLOBE, HARD_DRIVES, HASH, KEY, MAGNIFYING_GLASS,
    PENCIL_SIMPLE, PLUGS_CONNECTED, PLUS, SHIELD_CHECK, TERMINAL, TRASH, UPLOAD_SIMPLE, USER,
};

use crate::config;

use super::state::{Action, Panel};
use super::theme::{
    ACCENT, ACCENT_HOVER, BORDER, CARD_BG, MUTED, TEXT, badge, card, danger_button, danger_card,
    empty_state, icon_text, primary_button, secondary_button, section_heading,
};

impl Panel {
    pub(super) fn host_list(&mut self, ui: &mut Ui) {
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

    pub(super) fn host_details(&mut self, ui: &mut Ui) -> Option<Action> {
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
            Color32::from_rgba_unmultiplied(235, 238, 244, 12)
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
