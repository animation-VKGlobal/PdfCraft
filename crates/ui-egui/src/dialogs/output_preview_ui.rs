//! Print production ▸ Output preview (Acrobat's Output Preview inspector).
//!
//! Simulation profile, overprint preview, color separations (CMYK & spot plates),
//! live coverage sampling at the pointer, and total area coverage warnings.

use egui::{Align, Color32, CornerRadius, Layout, Sense, Stroke, vec2};

use crate::theme::{self, Tokens};
use crate::{Dialog, PdfCraftApp, widgets};

/// Settings and live inspection values of the Output Preview inspector.
#[derive(Clone, Debug, PartialEq)]
pub struct OutputPreviewDraft {
    pub simulation_profile: String,
    pub simulate_overprinting: bool,
    pub simulate_paper_color: bool,
    pub simulate_black_ink: bool,
    pub set_page_bg_color: bool,
    pub page_bg_color: [u8; 3],
    pub show_filter: String,
    pub warning_opacity: u32,
    pub show_boxes: bool,
    pub preview_mode: String,
    pub process_plates: bool,
    pub plate_cyan: bool,
    pub plate_magenta: bool,
    pub plate_yellow: bool,
    pub plate_black: bool,
    pub sample_cyan: f32,
    pub sample_magenta: f32,
    pub sample_yellow: f32,
    pub sample_black: f32,
    pub sample_size: String,
    pub total_area_coverage_warn: bool,
    pub total_area_coverage_limit: u32,
    pub page_has_transparency: bool,
    pub transparency_blending_space: String,
    pub last_sample_pos: Option<(usize, [f32; 2])>,
}

impl Default for OutputPreviewDraft {
    fn default() -> Self {
        Self {
            simulation_profile: "U.S. Web Coated (SWOP) v2".into(),
            simulate_overprinting: true,
            simulate_paper_color: false,
            simulate_black_ink: false,
            set_page_bg_color: false,
            page_bg_color: [255, 255, 255],
            show_filter: "All".into(),
            warning_opacity: 100,
            show_boxes: false,
            preview_mode: "Separations".into(),
            process_plates: true,
            plate_cyan: true,
            plate_magenta: true,
            plate_yellow: true,
            plate_black: true,
            sample_cyan: 0.0,
            sample_magenta: 0.0,
            sample_yellow: 0.0,
            sample_black: 0.0,
            sample_size: "Point Sample".into(),
            total_area_coverage_warn: false,
            total_area_coverage_limit: 280,
            page_has_transparency: false,
            transparency_blending_space: "DeviceCMYK".into(),
            last_sample_pos: None,
        }
    }
}

impl OutputPreviewDraft {
    /// Update sampled CMYK values from standard RGB (0-255).
    pub fn update_sample_rgb(&mut self, r: u8, g: u8, b: u8) {
        let rf = r as f32 / 255.0;
        let gf = g as f32 / 255.0;
        let bf = b as f32 / 255.0;
        let k = (1.0 - rf.max(gf).max(bf)).clamp(0.0, 1.0);
        let ucr = (1.0 - k * 0.4).clamp(0.0, 1.0);
        self.sample_cyan = (((1.0 - rf) * ucr).clamp(0.0, 1.0) * 100.0).round();
        self.sample_magenta = (((1.0 - gf) * ucr).clamp(0.0, 1.0) * 100.0).round();
        self.sample_yellow = (((1.0 - bf) * ucr).clamp(0.0, 1.0) * 100.0).round();
        self.sample_black = (k * 100.0).round();
    }

    /// Request tag bits encoding the active plate isolation and TAC warning configuration.
    pub fn filter_tag(&self) -> u64 {
        let is_color_warn = self.preview_mode == "Color Warnings";
        let is_separations = self.preview_mode == "Separations";
        let plates_filtered = is_separations && (!self.plate_cyan || !self.plate_magenta || !self.plate_yellow || !self.plate_black);

        if !plates_filtered && !self.total_area_coverage_warn && !is_color_warn {
            return 0;
        }

        let mut tag = crate::canvas::OP_ACTIVE;
        if is_separations {
            if !self.plate_cyan {
                tag |= crate::canvas::OP_NO_CYAN;
            }
            if !self.plate_magenta {
                tag |= crate::canvas::OP_NO_MAGENTA;
            }
            if !self.plate_yellow {
                tag |= crate::canvas::OP_NO_YELLOW;
            }
            if !self.plate_black {
                tag |= crate::canvas::OP_NO_BLACK;
            }
        }
        if self.total_area_coverage_warn {
            tag |= crate::canvas::OP_TAC_WARN;
            let limit = (u64::from(self.total_area_coverage_limit) & crate::canvas::OP_TAC_LIMIT_MASK) << crate::canvas::OP_TAC_LIMIT_SHIFT;
            tag |= limit;
        }
        if is_color_warn {
            tag |= crate::canvas::OP_COLOR_WARNINGS;
        }
        tag
    }
}

fn color_swatch(ui: &mut egui::Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(13.0, 13.0), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(2), color);
    ui.painter().rect_stroke(rect, CornerRadius::same(2), Stroke::new(1.0, Color32::from_gray(120)), egui::StrokeKind::Outside);
}

/// Draw the floating Output Preview inspector window.
pub fn show(app: &mut PdfCraftApp, ctx: &egui::Context) {
    let mut is_open = true;
    let t = Tokens::get(ctx);

    let mut open_page_boxes = false;
    let initial_tag = app.output_preview_draft.filter_tag();
    let initial_paper_color =
        (app.output_preview_draft.simulate_paper_color, app.output_preview_draft.set_page_bg_color, app.output_preview_draft.page_bg_color);

    egui::Window::new(tl!("Output Preview"))
        .id(egui::Id::unique("output_preview_window"))
        .open(&mut is_open)
        .collapsible(false)
        .resizable(true)
        .order(egui::Order::Foreground)
        .min_width(380.0)
        .default_width(420.0)
        .default_pos(egui::pos2(120.0, 80.0))
        .show(ctx, |ui| {
            widgets::begin_dialog(ui);
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                let d = &mut app.output_preview_draft;

                // ---- Simulate section --------------------------------------------------------
                widgets::section_title(ui, tl!("Simulate"));
                ui.horizontal(|ui| {
                    ui.label(tl!("Simulation Profile:"));
                    craft_ui::ComboBox::from_id_salt("op_sim_profile").selected_text(&d.simulation_profile).show_ui(ui, |ui| {
                        for profile in [
                            "U.S. Web Coated (SWOP) v2",
                            "Coated FOGRA39 (ISO 12647-2:2004)",
                            "GRACoL 2006 (ISO 12647-2:2004)",
                            "Euroscale Uncoated v2",
                            "Japan Color 2001 Coated",
                            "sRGB IEC61966-2.1",
                            "Apple RGB",
                            "Adobe RGB (1998)",
                        ] {
                            ui.selectable_value(&mut d.simulation_profile, profile.into(), profile);
                        }
                    });
                });
                ui.add_space(3.0);

                ui.horizontal(|ui| {
                    ui.add(widgets::checkbox(&mut d.simulate_overprinting, tl!("Simulate Overprinting")));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let has_op = if d.simulate_overprinting { tl!("Yes") } else { tl!("No") };
                        ui.label(
                            egui::RichText::new(format!("{}: {has_op}", tl!("Page has Overprint"))).color(t.text_muted).font(theme::regular(12.0)),
                        );
                    });
                });
                ui.add_space(3.0);

                ui.horizontal(|ui| {
                    ui.add(widgets::checkbox(&mut d.simulate_paper_color, tl!("Simulate Paper Color")));
                    ui.add(widgets::checkbox(&mut d.set_page_bg_color, tl!("Set Page Background Color")));
                    color_swatch(ui, Color32::from_rgb(d.page_bg_color[0], d.page_bg_color[1], d.page_bg_color[2]));
                });
                ui.add_space(3.0);

                ui.horizontal(|ui| {
                    ui.add(widgets::checkbox(&mut d.simulate_black_ink, tl!("Simulate Black Ink")));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let _ = ui.button(tl!("Ink Manager…"));
                    });
                });
                ui.add_space(8.0);
                ui.separator();

                // ---- Show section ------------------------------------------------------------
                widgets::section_title(ui, tl!("Show"));
                ui.horizontal(|ui| {
                    ui.label(tl!("Show:"));
                    craft_ui::ComboBox::from_id_salt("op_show_filter").selected_text(&d.show_filter).show_ui(ui, |ui| {
                        for f in ["All", "DeviceCMYK", "DeviceRGB", "DeviceGray", "Images", "Text", "Line Art"] {
                            ui.selectable_value(&mut d.show_filter, f.into(), f);
                        }
                    });
                    ui.add_space(8.0);
                    ui.label(tl!("Warning Opacity:"));
                    ui.add(egui::DragValue::new(&mut d.warning_opacity).range(0..=100).suffix("%"));
                });
                ui.add_space(3.0);

                ui.horizontal(|ui| {
                    ui.add(widgets::checkbox(&mut d.show_boxes, tl!("Show art, trim, & bleed boxes")));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button(tl!("Set Page Boxes")).clicked() {
                            open_page_boxes = true;
                        }
                    });
                });
                ui.add_space(8.0);
                ui.separator();

                // ---- Preview dropdown -------------------------------------------------------
                ui.horizontal(|ui| {
                    ui.label(tl!("Preview:"));
                    craft_ui::ComboBox::from_id_salt("op_preview_mode").selected_text(&d.preview_mode).show_ui(ui, |ui| {
                        for mode in ["Separations", "Color Warnings", "Object Inspector"] {
                            ui.selectable_value(&mut d.preview_mode, mode.into(), mode);
                        }
                    });
                });
                ui.add_space(6.0);

                // ---- Separations Table -------------------------------------------------------
                ui.label(egui::RichText::new(tl!("Separations")).font(theme::semibold(13.0)));
                egui::Frame::NONE
                    .fill(t.field)
                    .stroke(Stroke::new(1.0, t.border))
                    .corner_radius(CornerRadius::same(4))
                    .inner_margin(egui::Margin::same(8))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());

                        // Process Plates master row
                        let mut proc = d.process_plates;
                        if ui.add(widgets::checkbox(&mut proc, tl!("Process Plates"))).changed() {
                            d.process_plates = proc;
                            d.plate_cyan = proc;
                            d.plate_magenta = proc;
                            d.plate_yellow = proc;
                            d.plate_black = proc;
                        }
                        ui.add_space(2.0);

                        // Cyan
                        ui.horizontal(|ui| {
                            ui.add(widgets::checkbox_without_text(&mut d.plate_cyan));
                            color_swatch(ui, Color32::from_rgb(0, 174, 239));
                            ui.label(tl!("Process Cyan"));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(format!("{:.0}%", d.sample_cyan));
                            });
                        });
                        ui.add_space(2.0);

                        // Magenta
                        ui.horizontal(|ui| {
                            ui.add(widgets::checkbox_without_text(&mut d.plate_magenta));
                            color_swatch(ui, Color32::from_rgb(236, 0, 140));
                            ui.label(tl!("Process Magenta"));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(format!("{:.0}%", d.sample_magenta));
                            });
                        });
                        ui.add_space(2.0);

                        // Yellow
                        ui.horizontal(|ui| {
                            ui.add(widgets::checkbox_without_text(&mut d.plate_yellow));
                            color_swatch(ui, Color32::from_rgb(255, 242, 0));
                            ui.label(tl!("Process Yellow"));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(format!("{:.0}%", d.sample_yellow));
                            });
                        });
                        ui.add_space(2.0);

                        // Black
                        ui.horizontal(|ui| {
                            ui.add(widgets::checkbox_without_text(&mut d.plate_black));
                            color_swatch(ui, Color32::from_rgb(0, 0, 0));
                            ui.label(tl!("Process Black"));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(format!("{:.0}%", d.sample_black));
                            });
                        });
                        ui.add_space(4.0);

                        // Total Area Coverage sum
                        let total = d.sample_cyan + d.sample_magenta + d.sample_yellow + d.sample_black;
                        ui.horizontal(|ui| {
                            ui.add_space(32.0);
                            ui.label(egui::RichText::new(tl!("Total Area Coverage")).font(theme::semibold(12.0)));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(egui::RichText::new(format!("{total:.0}%")).font(theme::semibold(12.0)));
                            });
                        });
                    });
                ui.add_space(8.0);

                // ---- Sample Size & TAC limit -------------------------------------------------
                ui.horizontal(|ui| {
                    ui.label(tl!("Sample Size:"));
                    craft_ui::ComboBox::from_id_salt("op_sample_size").selected_text(&d.sample_size).show_ui(ui, |ui| {
                        for s in ["Point Sample", "3 by 3 Average", "5 by 5 Average"] {
                            ui.selectable_value(&mut d.sample_size, s.into(), s);
                        }
                    });
                });
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.add(widgets::checkbox(&mut d.total_area_coverage_warn, tl!("Total Area Coverage")));
                    color_swatch(ui, Color32::from_rgb(0, 255, 0));
                    ui.add(egui::DragValue::new(&mut d.total_area_coverage_limit).range(100..=400).suffix("%"));
                });
                ui.add_space(6.0);

                let has_trans = if d.page_has_transparency { tl!("Yes") } else { tl!("No") };
                ui.label(
                    egui::RichText::new(format!("{}: {has_trans}", tl!("Page has Transparency"))).color(t.text_muted).font(theme::regular(12.0)),
                );
                ui.label(
                    egui::RichText::new(format!("{}: {}", tl!("Transparency Blending Color Space"), d.transparency_blending_space))
                        .color(t.text_muted)
                        .font(theme::regular(12.0)),
                );
            });
        });

    let tag_changed = app.output_preview_draft.filter_tag() != initial_tag;
    let paper_changed =
        (app.output_preview_draft.simulate_paper_color, app.output_preview_draft.set_page_bg_color, app.output_preview_draft.page_bg_color)
            != initial_paper_color;

    if tag_changed || paper_changed {
        app.invalidate_output_preview();
    }

    if open_page_boxes {
        app.boxes_draft.range = crate::dialogs::pageboxes::Range::Current;
        app.boxes_draft.seeded = None;
        app.dialog = Some(Dialog::PageBoxes);
    } else if !is_open {
        let had_filter = app.output_preview_draft.filter_tag() != 0;
        app.dialog = None;
        if had_filter {
            app.invalidate_output_preview();
        }
    }
}

/// Paint page boxes overlay (ArtBox, TrimBox, BleedBox, CropBox) for Output Preview.
pub(crate) fn paint_output_preview_boxes(
    painter: &egui::Painter,
    xf: &crate::canvas::PageXform,
    info: &pdfcraft_render::DocInfo,
    doc: &pdfcraft_engine::Document,
    page: usize,
) {
    let boxes = doc.page_boxes();
    let Some(page_boxes) = boxes.get(page) else { return };
    let crop = page_boxes[1];
    let bleed = page_boxes[2];
    let trim = page_boxes[3];
    let art = page_boxes[4];

    let items = [
        ("CropBox", crop, Color32::from_rgb(136, 23, 152)),
        ("BleedBox", bleed, Color32::from_rgb(0, 120, 215)),
        ("TrimBox", trim, Color32::from_rgb(16, 137, 62)),
        ("ArtBox", art, Color32::from_rgb(216, 59, 1)),
    ];

    let mut drawn_rects: Vec<(egui::Rect, String, Color32)> = Vec::new();

    for (name, b, color) in items {
        let r = xf.user_rect_f64(info, page, b);
        if !r.is_positive() {
            continue;
        }
        if let Some(existing) =
            drawn_rects.iter_mut().find(|(prev, _, _)| (prev.min - r.min).length_sq() < 1.0 && (prev.max - r.max).length_sq() < 1.0)
        {
            existing.1.push_str(&format!(", {name}"));
        } else {
            drawn_rects.push((r, name.to_string(), color));
        }
    }

    for (r, label, color) in drawn_rects {
        let corners = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
        let stroke = Stroke::new(1.5, color);
        painter.extend(egui::Shape::dashed_line(&corners, stroke, 6.0, 3.0));

        let label_pos = r.left_top() + vec2(4.0, 3.0);
        painter.text(label_pos, egui::Align2::LEFT_TOP, label, theme::semibold(10.5), color);
    }
}

impl PdfCraftApp {
    /// Invalidate view content so rasters update with Output Preview separation/TAC filters.
    pub(crate) fn invalidate_output_preview(&mut self) {
        for v in &mut self.views {
            v.invalidate_content();
        }
    }

    /// Sample the color under the cursor for Output Preview live CMYK readout.
    pub(crate) fn sample_output_preview_color(&mut self, index: usize, page: usize, pos: egui::Pos2, _page_rect: egui::Rect) {
        let Some(view) = self.views.get(index) else { return };
        let Some(doc) = self.session.get(view.id) else { return };
        let Some((_, xf)) = view.screen_xforms.iter().find(|(p, _)| *p == page).copied() else { return };

        let (u, v) = xf.screen_to_norm(pos);
        let vx = u * xf.pw;
        let vy = v * xf.ph;

        let radius: f32 = match self.output_preview_draft.sample_size.as_str() {
            "3 by 3 Average" => 1.5,
            "5 by 5 Average" => 2.5,
            _ => 0.5,
        };

        let view_rect =
            [(vx - radius).clamp(0.0, xf.pw), (vy - radius).clamp(0.0, xf.ph), (vx + radius).clamp(0.0, xf.pw), (vy + radius).clamp(0.0, xf.ph)];

        if view_rect[2] <= view_rect[0] || view_rect[3] <= view_rect[1] {
            return;
        }

        if let Ok((w, h, rgba)) = self.render_area(index, page, view_rect)
            && w > 0
            && h > 0
        {
            let slice: &[u8] = &rgba;
            let mut total_r: u64 = 0;
            let mut total_g: u64 = 0;
            let mut total_b: u64 = 0;
            let mut count: u64 = 0;

            for chunk in slice.as_chunks::<4>().0 {
                let a = chunk[3] as f32;
                if a > 0.0 {
                    let r = (chunk[0] as f32 * 255.0 / a).min(255.0).round() as u64;
                    let g = (chunk[1] as f32 * 255.0 / a).min(255.0).round() as u64;
                    let b = (chunk[2] as f32 * 255.0 / a).min(255.0).round() as u64;
                    total_r += r;
                    total_g += g;
                    total_b += b;
                    count += 1;
                } else {
                    total_r += 255;
                    total_g += 255;
                    total_b += 255;
                    count += 1;
                }
            }

            if let (Some(avg_r), Some(avg_g), Some(avg_b)) = (total_r.checked_div(count), total_g.checked_div(count), total_b.checked_div(count)) {
                self.output_preview_draft.update_sample_rgb(avg_r.min(255) as u8, avg_g.min(255) as u8, avg_b.min(255) as u8);
            }
        }

        self.output_preview_draft.page_has_transparency = doc.page_uses_transparency(page);
        if let Some(cs) = doc.page_transparency_blending_space(page) {
            self.output_preview_draft.transparency_blending_space = cs;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_preview_default_state() {
        let d = OutputPreviewDraft::default();
        assert_eq!(d.simulation_profile, "U.S. Web Coated (SWOP) v2");
        assert!(d.simulate_overprinting);
        assert!(!d.simulate_paper_color);
        assert!(!d.simulate_black_ink);
        assert_eq!(d.show_filter, "All");
        assert_eq!(d.preview_mode, "Separations");
        assert!(d.process_plates);
        assert!(d.plate_cyan);
        assert!(d.plate_magenta);
        assert!(d.plate_yellow);
        assert!(d.plate_black);
        assert_eq!(d.sample_size, "Point Sample");
        assert_eq!(d.total_area_coverage_limit, 280);
    }

    #[test]
    fn cmyk_rgb_conversion_sampling() {
        let mut d = OutputPreviewDraft::default();
        // Pure White (255, 255, 255) -> CMYK 0, 0, 0, 0
        d.update_sample_rgb(255, 255, 255);
        assert_eq!(d.sample_cyan, 0.0);
        assert_eq!(d.sample_magenta, 0.0);
        assert_eq!(d.sample_yellow, 0.0);
        assert_eq!(d.sample_black, 0.0);

        // Pure Black (0, 0, 0) -> CMYK 60, 60, 60, 100 (standard rich black, TAC 280%)
        d.update_sample_rgb(0, 0, 0);
        assert_eq!(d.sample_black, 100.0);
        assert_eq!(d.sample_cyan, 60.0);
        assert_eq!(d.sample_magenta, 60.0);
        assert_eq!(d.sample_yellow, 60.0);

        // Pure Cyan (0, 255, 255) -> Cyan 100%, M 0%, Y 0%, K 0%
        d.update_sample_rgb(0, 255, 255);
        assert_eq!(d.sample_cyan, 100.0);
        assert_eq!(d.sample_magenta, 0.0);
        assert_eq!(d.sample_yellow, 0.0);
        assert_eq!(d.sample_black, 0.0);

        // Pure Magenta (255, 0, 255) -> Cyan 0%, M 100%, Y 0%, K 0%
        d.update_sample_rgb(255, 0, 255);
        assert_eq!(d.sample_cyan, 0.0);
        assert_eq!(d.sample_magenta, 100.0);
        assert_eq!(d.sample_yellow, 0.0);
        assert_eq!(d.sample_black, 0.0);

        // Pure Yellow (255, 255, 0) -> Cyan 0%, M 0%, Y 100%, K 0%
        d.update_sample_rgb(255, 255, 0);
        assert_eq!(d.sample_cyan, 0.0);
        assert_eq!(d.sample_magenta, 0.0);
        assert_eq!(d.sample_yellow, 100.0);
        assert_eq!(d.sample_black, 0.0);
    }

    #[test]
    fn sample_size_modes_and_cmyk_averages() {
        let mut d = OutputPreviewDraft::default();
        assert_eq!(d.sample_size, "Point Sample");
        assert!(d.last_sample_pos.is_none());

        d.sample_size = "3 by 3 Average".into();
        assert_eq!(d.sample_size, "3 by 3 Average");

        d.sample_size = "5 by 5 Average".into();
        assert_eq!(d.sample_size, "5 by 5 Average");

        // 50% Gray (128, 128, 128)
        d.update_sample_rgb(128, 128, 128);
        assert_eq!(d.sample_cyan, 40.0);
        assert_eq!(d.sample_magenta, 40.0);
        assert_eq!(d.sample_yellow, 40.0);
        assert_eq!(d.sample_black, 50.0);

        // Dark blend
        d.update_sample_rgb(30, 40, 50);
        assert!(d.sample_black >= 80.0);
        assert!(d.sample_cyan + d.sample_magenta + d.sample_yellow + d.sample_black > 80.0);
    }

    #[test]
    fn filter_tag_and_plate_isolation() {
        let mut d = OutputPreviewDraft::default();
        // Default state: all plates active, TAC off -> filter_tag is 0
        assert_eq!(d.filter_tag(), 0);

        // Turn off Cyan
        d.plate_cyan = false;
        let tag_no_cyan = d.filter_tag();
        assert_ne!(tag_no_cyan, 0);
        assert_ne!(tag_no_cyan & crate::canvas::OP_ACTIVE, 0);
        assert_ne!(tag_no_cyan & crate::canvas::OP_NO_CYAN, 0);

        // Turn on TAC warning
        d.plate_cyan = true;
        d.total_area_coverage_warn = true;
        d.total_area_coverage_limit = 250;
        let tag_tac = d.filter_tag();
        assert_ne!(tag_tac, 0);
        assert_ne!(tag_tac & crate::canvas::OP_TAC_WARN, 0);
        let extracted_limit = (tag_tac >> crate::canvas::OP_TAC_LIMIT_SHIFT) & crate::canvas::OP_TAC_LIMIT_MASK;
        assert_eq!(extracted_limit, 250);

        // Test apply_output_preview_filter on image pixels
        let mut img = egui::ColorImage::new(
            [2, 1],
            vec![Color32::from_rgba_premultiplied(0, 255, 255, 255), Color32::from_rgba_premultiplied(10, 10, 10, 255)],
        );

        // Filter with Cyan turned off
        crate::canvas::apply_output_preview_filter(&mut img, tag_no_cyan);
        // The first pixel was pure cyan: without cyan ink it becomes white (255, 255, 255)
        assert_eq!(img.pixels[0], Color32::from_rgba_premultiplied(255, 255, 255, 255));

        // Filter with TAC warning on high-density second pixel
        let mut img_tac = egui::ColorImage::new([1, 1], vec![Color32::from_rgba_premultiplied(10, 10, 10, 255)]);
        crate::canvas::apply_output_preview_filter(&mut img_tac, tag_tac);
        // Exceeds 250% coverage -> neon green warning (0, 255, 0)
        assert_eq!(img_tac.pixels[0], Color32::from_rgba_premultiplied(0, 255, 0, 255));
    }
}
