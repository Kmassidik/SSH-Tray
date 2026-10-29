use eframe::egui::style::WidgetVisuals;
use eframe::egui::{
    self, Color32, Context, CornerRadius, FontFamily, FontId, Frame, Margin, RichText, Stroke,
    TextStyle, Ui,
};
use egui_phosphor::regular::TERMINAL_WINDOW;

pub(super) const APP_BG: Color32 = Color32::from_rgb(10, 11, 14);
pub(super) const SIDEBAR_BG: Color32 = Color32::from_rgb(15, 17, 21);
pub(super) const CARD_BG: Color32 = Color32::from_rgb(20, 22, 27);
pub(super) const RAISED_BG: Color32 = Color32::from_rgb(27, 30, 36);
pub(super) const BORDER: Color32 = Color32::from_rgb(43, 47, 56);
pub(super) const TEXT: Color32 = Color32::from_rgb(235, 238, 244);
pub(super) const MUTED: Color32 = Color32::from_rgb(145, 153, 166);
pub(super) const ACCENT: Color32 = Color32::from_rgb(225, 228, 234);
pub(super) const ACCENT_HOVER: Color32 = Color32::from_rgb(255, 255, 255);
pub(super) const SUCCESS: Color32 = Color32::from_rgb(79, 198, 139);
pub(super) const ERROR: Color32 = Color32::from_rgb(244, 113, 122);

pub(super) fn configure_theme(ctx: &Context) {
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
    style.visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(235, 238, 244, 60);
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
        fg_stroke: Stroke::new(1.0_f32, APP_BG),
        expansion: 0.0,
    };
    style.visuals.widgets.open = style.visuals.widgets.hovered;
    ctx.set_style(style);
}

pub(super) fn icon_text(icon: &str, text: &str) -> String {
    format!("{icon}  {text}")
}

pub(super) fn primary_button(text: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text.into()).strong().color(APP_BG))
        .fill(ACCENT)
        .stroke(Stroke::new(1.0_f32, ACCENT_HOVER))
        .corner_radius(CornerRadius::same(7))
        .min_size(egui::vec2(0.0, 34.0))
}

pub(super) fn secondary_button(text: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text.into()).color(TEXT))
        .fill(RAISED_BG)
        .stroke(Stroke::new(1.0_f32, BORDER))
        .corner_radius(CornerRadius::same(7))
        .min_size(egui::vec2(0.0, 34.0))
}

pub(super) fn danger_button(text: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text.into()).strong().color(ERROR))
        .fill(Color32::from_rgba_unmultiplied(244, 113, 122, 18))
        .stroke(Stroke::new(
            1.0_f32,
            Color32::from_rgba_unmultiplied(244, 113, 122, 95),
        ))
        .corner_radius(CornerRadius::same(7))
        .min_size(egui::vec2(0.0, 34.0))
}

pub(super) fn tab_button(ui: &mut Ui, selected: bool, icon: &str, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(
            RichText::new(icon_text(icon, text))
                .strong()
                .color(if selected { TEXT } else { MUTED }),
        )
        .fill(if selected {
            Color32::from_rgba_unmultiplied(235, 238, 244, 16)
        } else {
            Color32::TRANSPARENT
        })
        .stroke(if selected {
            Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(235, 238, 244, 120))
        } else {
            Stroke::NONE
        })
        .corner_radius(CornerRadius::same(7))
        .min_size(egui::vec2(0.0, 36.0)),
    )
}

pub(super) fn brand_mark(ui: &mut Ui) {
    Frame::new()
        .fill(ACCENT)
        .corner_radius(CornerRadius::same(9))
        .inner_margin(Margin::same(10))
        .show(ui, |ui| {
            ui.label(RichText::new(TERMINAL_WINDOW).size(19.0).color(APP_BG));
        });
}

pub(super) fn badge(ui: &mut Ui, text: &str) {
    Frame::new()
        .fill(RAISED_BG)
        .stroke(Stroke::new(1.0_f32, BORDER))
        .corner_radius(CornerRadius::same(5))
        .inner_margin(Margin::symmetric(7, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(text).small().strong().color(MUTED));
        });
}

pub(super) fn card(ui: &mut Ui, contents: impl FnOnce(&mut Ui)) {
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

pub(super) fn danger_card(ui: &mut Ui, contents: impl FnOnce(&mut Ui)) {
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

pub(super) fn section_heading(ui: &mut Ui, icon: &str, text: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(icon).size(16.0).color(ACCENT_HOVER));
        ui.label(RichText::new(text).size(16.0).strong().color(TEXT));
    });
}

pub(super) fn empty_state(ui: &mut Ui, icon: &str, title: &str, description: &str) {
    ui.vertical_centered(|ui| {
        ui.add_space(12.0);
        ui.label(RichText::new(icon).size(32.0).color(MUTED));
        ui.add_space(6.0);
        ui.label(RichText::new(title).size(18.0).strong().color(TEXT));
        ui.label(RichText::new(description).color(MUTED));
        ui.add_space(12.0);
    });
}
