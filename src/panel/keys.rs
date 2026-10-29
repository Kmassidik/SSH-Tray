use eframe::egui::{
    Color32, CornerRadius, Frame, Label, Margin, RichText, ScrollArea, Sense, Stroke, Ui,
};
use egui_phosphor::regular::{
    COPY, DOWNLOAD_SIMPLE, HASH, INFO, KEY, LOCK_KEY, PENCIL_SIMPLE, SHIELD_CHECK, SPARKLE,
    TERMINAL_WINDOW, TRASH,
};

use super::state::{Action, Panel};
use super::theme::{
    ACCENT, ACCENT_HOVER, APP_BG, BORDER, CARD_BG, MUTED, TEXT, badge, card, danger_button,
    danger_card, empty_state, icon_text, primary_button, secondary_button, section_heading,
};

impl Panel {
    pub(super) fn key_list(&mut self, ui: &mut Ui) {
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
                            Color32::from_rgba_unmultiplied(235, 238, 244, 12)
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

    pub(super) fn key_details(&mut self, ui: &mut Ui) -> Option<Action> {
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
            eframe::egui::Grid::new("public_key_metadata")
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
}
