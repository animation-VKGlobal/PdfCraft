use super::geometry::{drawn_text_box_rect, page_rotation, text_box_rect};
use super::paint::{edited_in_view, new_comment_styled};
use super::state::{CharStyle, ComposerKind, PageCx};
use super::{
    Color32, CommentPrefs, CommentTool, CornerRadius, DocInfo, DocView, Edit, LineEnding, Markup, NoteIcon, PagePointer, QuickTool, Shape, Tokens,
    pos2, theme,
};
use crate::page_keys::PageKeys;

/// The floating composer card (new note, new text box, edit). Returns the edit to apply.
pub(crate) fn composer(ctx: &egui::Context, keys: PageKeys, view: &mut DocView, info: &DocInfo, prefs: &CommentPrefs) -> Option<Edit> {
    let c = view.comments.composer.as_ref()?;
    let page = c.page;
    let Some(xf) = view.page_xform(page) else {
        // Scrolled away: keep the draft until the page is back, and give it the keyboard then.
        if let Some(c) = view.comments.composer.as_mut() {
            c.focus = true;
        }
        return None;
    };
    let cx = PageCx {
        page,
        xf: &xf,
        ptr: &PagePointer::default(),
        info,
        tool: QuickTool::Select,
        prefs,
        allowed: true,
        hidden: false,
        redact_mark: ([0.89, 0.13, 0.13], 1.0),
        keys,
    };
    let anchor = cx.to_screen(c.at);
    let viewport = view.viewport_rect();
    let t = Tokens::get(ctx);
    let mut post = false;
    let mut cancel = false;
    let c = view.comments.composer.as_mut()?;
    let title = match c.kind {
        ComposerKind::Note => "Sticky note",
        ComposerKind::TextNote { .. } => "Add comment",
        ComposerKind::TextBox { .. } => "Text box",
        ComposerKind::Callout { .. } => "Callout",
        ComposerKind::Caret { .. } => "Inserted text",
        ComposerKind::Replace => "Replacement text",
        ComposerKind::Edit(_) => "Edit comment",
    };
    // Its comment scrolled out of the document: keep the draft until it is back, rather than
    // pushing the card into the window over the mode bar and toolbar (#4021). An edited comment
    // that is only partly scrolled away keeps its card, at the nearest point inside the document.
    let mut anchor = anchor;
    if !viewport.contains(anchor) {
        let part_visible = matches!(c.kind, ComposerKind::Edit(index) if edited_in_view(&cx, index, viewport));
        if !part_visible {
            c.focus = true;
            return None;
        }
        anchor = anchor.clamp(viewport.min, viewport.max);
    }
    let pos = pos2(anchor.x + 12.0, anchor.y);
    let area = egui::Area::new(egui::Id::unique(("comment-composer", view.pane.key))).order(egui::Order::Foreground).constrain_to(viewport);
    area.fixed_pos(pos).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).inner_margin(egui::Margin::same(12)).corner_radius(CornerRadius::same(8)).show(ui, |ui| {
            ui.set_width(260.0);
            let text_id = ui.scope_id().with("composer-text");
            sync_char_styles(&c.text, &mut c.styles, c.bold, c.italic);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(prefs.author.clone()).font(theme::semibold(13.0)));
                ui.label(egui::RichText::new(tl!(title)).font(theme::regular(11.5)).color(t.text_faint));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let btn_i = egui::Button::new(egui::RichText::new("I").italics().font(theme::regular(12.0)))
                        .min_size(egui::vec2(22.0, 20.0))
                        .fill(if c.italic { t.accent.gamma_multiply(0.2) } else { Color32::TRANSPARENT })
                        .stroke(if c.italic { egui::Stroke::new(1.0, t.accent) } else { egui::Stroke::NONE });
                    if ui.add(btn_i).on_hover_text(tl!("Italic (Ctrl+I)")).clicked() {
                        let sel = get_text_selection(ui.ctx(), text_id);
                        c.undo_text = Some((c.text.clone(), c.styles.clone()));
                        toggle_italic_range(&mut c.styles, sel, &mut c.italic);
                    }
                    let btn_b = egui::Button::new(egui::RichText::new("B").font(theme::semibold(12.0)))
                        .min_size(egui::vec2(22.0, 20.0))
                        .fill(if c.bold { t.accent.gamma_multiply(0.2) } else { Color32::TRANSPARENT })
                        .stroke(if c.bold { egui::Stroke::new(1.0, t.accent) } else { egui::Stroke::NONE });
                    if ui.add(btn_b).on_hover_text(tl!("Bold (Ctrl+B)")).clicked() {
                        let sel = get_text_selection(ui.ctx(), text_id);
                        c.undo_text = Some((c.text.clone(), c.styles.clone()));
                        toggle_bold_range(&mut c.styles, sel, &mut c.bold);
                    }
                });
            });
            ui.add_space(6.0);
            let hint = match c.kind {
                ComposerKind::TextBox { .. } | ComposerKind::Callout { .. } => tl!("Type text"),
                ComposerKind::Caret { .. } => tl!("Text to insert"),
                ComposerKind::Replace => tl!("Replacement text"),
                _ => tl!("Add a comment"),
            };
            let (is_bold, is_italic) = (c.bold, c.italic);
            let styles_ref = &c.styles;
            let mut layouter = move |ui: &egui::Ui, text: &dyn egui::TextBuffer, wrap: f32| {
                format_comment_galley_styled(ui, text.as_str(), 13.0, wrap, t.text, styles_ref, (is_bold, is_italic))
            };
            let edit = ui
                .add(
                    egui::TextEdit::multiline(&mut c.text)
                        .hint_text(hint)
                        .desired_rows(3)
                        .desired_width(f32::INFINITY)
                        .id(text_id)
                        .layouter(&mut layouter),
                )
                // 0.37 kittest rejects an unnamed text input; the hint is its logical name.
                .accessible_name(hint);
            let sel_key = text_id.with("saved_selection");
            if let Some(sel) = get_text_selection(ui.ctx(), text_id) {
                ui.data_mut(|d| d.insert_temp(sel_key, sel));
            } else if edit.clicked_by(egui::PointerButton::Primary) {
                ui.data_mut(|d| d.remove::<(usize, usize)>(sel_key));
            }
            if edit.secondary_clicked()
                && let Some((start, end)) = ui.data(|d| d.get_temp::<(usize, usize)>(sel_key))
            {
                let count = c.text.chars().count();
                if start < count && end <= count && start < end {
                    reselect_range(ui.ctx(), text_id, start, end);
                }
            }
            if c.focus {
                edit.request_focus();
                c.focus = false;
            }
            format_editor_keys(ui, &edit, &mut c.text, &mut c.styles, &mut c.undo_text, &mut c.bold, &mut c.italic);
            let (c_text, c_styles, c_undo, c_bold, c_italic) = (&mut c.text, &mut c.styles, &mut c.undo_text, &mut c.bold, &mut c.italic);
            edit.context_menu(|ui| {
                show_comment_text_context_menu(ui, text_id, c_text, c_styles, c_undo, c_bold, c_italic);
            });
            let enter = submit_key(ui, &edit, false);
            if keys.pressed(ui.ctx(), egui::Key::Escape) {
                cancel = true;
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let can_post = !c.text.trim().is_empty() || matches!(c.kind, ComposerKind::Edit(_));
                    if ui
                        .add_enabled(
                            can_post,
                            egui::Button::new(egui::RichText::new(tl!("Post")).color(Color32::WHITE)).fill(t.accent).corner_radius(14),
                        )
                        .clicked()
                        || (enter && can_post)
                    {
                        post = true;
                    }
                    if ui.add(egui::Button::new(tl!("Cancel")).corner_radius(14)).clicked() {
                        cancel = true;
                    }
                });
            });
        });
    });
    if cancel {
        view.comments.composer = None;
        return None;
    }
    if !post {
        return None;
    }
    let (all_bold, all_italic) = (!c.styles.is_empty() && c.styles.iter().all(|s| s.bold), !c.styles.is_empty() && c.styles.iter().all(|s| s.italic));
    let any_styling = c.styles.iter().any(|s| s.bold || s.italic || s.underline || s.color.is_some());
    let rc = if any_styling { Some(styles_to_rc(&c.text, &c.styles, c.bold, c.italic)) } else { None };
    let c = view.comments.composer.take()?;
    let typewriter_edit = matches!(c.kind, ComposerKind::Edit(index) if info.annotations.iter().any(|a|
        a.page == page && a.index == index && a.subtype == "FreeText" && a.intent.as_deref() == Some("FreeTextTypeWriter")
    ));
    let text = if typewriter_edit { c.text } else { c.text.trim_end().to_string() };
    match c.kind {
        ComposerKind::Edit(index) => {
            info.annotations.iter().any(|a| a.page == page && a.index == index).then_some(Edit::SetAnnotationContents { page, index, text, rc })
        }
        ComposerKind::Note => {
            view.comments.tool_done = true;
            Some(new_comment_styled(&cx, CommentTool::Note, Shape::Note { at: c.at, icon: NoteIcon::Comment }, text, all_bold, all_italic, rc))
        }
        ComposerKind::TextNote { quads, markup } => {
            view.comments.tool_done = true;
            let tool = match markup {
                Markup::Highlight => CommentTool::Highlight,
                Markup::Underline => CommentTool::Underline,
                Markup::StrikeOut => CommentTool::StrikeOut,
                Markup::Squiggly => CommentTool::Squiggly,
            };
            Some(new_comment_styled(&cx, tool, Shape::TextMarkup { kind: markup, quads }, text, all_bold, all_italic, rc))
        }
        ComposerKind::TextBox { size } => {
            view.comments.tool_done = true;
            let rotation = page_rotation(info, page);
            let border = prefs.style(CommentTool::TextBox).width;
            let font_size = prefs.font_size(CommentTool::TextBox).unwrap_or(12.0);
            let rect = match (size, info.pages.get(page)) {
                (Some(size), Some(p)) => drawn_text_box_rect(p, c.at, size, &text, font_size, border),
                _ => text_box_rect(c.at, &text, font_size, rotation),
            };
            Some(new_comment_styled(&cx, CommentTool::TextBox, Shape::TextBox { rect, font_size }, text, all_bold, all_italic, rc))
        }
        ComposerKind::Callout { point } => {
            view.comments.tool_done = true;
            let font_size = prefs.font_size(CommentTool::Callout).unwrap_or(10.0);
            let rect = text_box_rect(c.at, &text, font_size, 0);
            let knee = pdfcraft_engine::callout_knee(rect, point);
            Some(new_comment_styled(
                &cx,
                CommentTool::Callout,
                Shape::Callout { rect, knee, point, font_size, ending: LineEnding::OpenArrow },
                text,
                all_bold,
                all_italic,
                rc,
            ))
        }
        ComposerKind::Replace => {
            view.comments.tool_done = true;
            let quads = view.comments.replace.take()?;
            Some(Edit::ReplaceText {
                page,
                quads,
                text,
                author: prefs.author.clone(),
                strike: prefs.style(CommentTool::ReplaceText),
                caret: prefs.style(CommentTool::ReplaceText),
            })
        }
        ComposerKind::Caret { size } => {
            view.comments.tool_done = true;
            let [x, y] = c.at;
            let half = size / 2.0;
            Some(new_comment_styled(
                &cx,
                CommentTool::Caret,
                Shape::Caret { rect: [x - half, y - size, x + half, y] },
                text,
                all_bold,
                all_italic,
                rc,
            ))
        }
    }
}

/// Submit only from the focused comment editor. Match each key event's modifiers exactly,
/// so Shift+Enter stays a newline and another editor cannot submit this draft. Replies also
/// keep their existing unmodified Enter-to-post behavior.
pub(crate) fn submit_key(ui: &mut egui::Ui, field: &egui::Response, plain_enter: bool) -> bool {
    field.has_focus()
        && ui.input_mut(|input| {
            let mut submit = false;
            input.events.retain(|event| {
                let matched = matches!(event,
                    egui::Event::Key { key: egui::Key::Enter, pressed: true, modifiers, .. }
                        if modifiers.matches_exact(egui::Modifiers::CTRL)
                            || modifiers.matches_exact(egui::Modifiers::COMMAND)
                            || plain_enter && modifiers.matches_exact(egui::Modifiers::NONE)
                );
                submit |= matched;
                !matched
            });
            submit
        })
}

/// Sync character styling vector length to match the character count of `text`.
pub(crate) fn sync_char_styles(text: &str, styles: &mut Vec<CharStyle>, default_bold: bool, default_italic: bool) {
    let char_count = text.chars().count();
    let def = CharStyle { bold: default_bold, italic: default_italic, underline: false, color: None };
    if styles.len() < char_count {
        let last = styles.last().copied().unwrap_or(def);
        styles.resize(char_count, last);
    } else if styles.len() > char_count {
        styles.truncate(char_count);
    }
}

/// Retrieve the active character selection range (start, end) in an egui TextEdit.
pub(crate) fn get_text_selection(ctx: &egui::Context, id: egui::Id) -> Option<(usize, usize)> {
    let state = egui::TextEdit::load_state(ctx, id)?;
    let range = state.cursor.char_range()?;
    let sorted = range.as_sorted_char_range();
    if !sorted.is_empty() { Some((sorted.start.into(), sorted.end.into())) } else { None }
}

/// Select a range of characters in an egui TextEdit.
pub(crate) fn reselect_range(ctx: &egui::Context, id: egui::Id, start: usize, end: usize) {
    if let Some(mut state) = egui::TextEdit::load_state(ctx, id) {
        state.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(start), egui::text::CCursor::new(end))));
        state.store(ctx, id);
    }
}

/// Move the caret in an egui TextEdit.
pub(crate) fn reposition_cursor(ctx: &egui::Context, id: egui::Id, pos: usize) {
    if let Some(mut state) = egui::TextEdit::load_state(ctx, id) {
        state.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(pos))));
        state.store(ctx, id);
    }
}

/// Toggle bold on the selected character range (or all if no selection).
pub(crate) fn toggle_bold_range(styles: &mut [CharStyle], selection: Option<(usize, usize)>, default_bold: &mut bool) {
    if let Some((start, end)) = selection {
        let slice = styles.get_mut(start..end.min(styles.len())).unwrap_or(&mut []);
        if !slice.is_empty() {
            let all_bold = slice.iter().all(|s| s.bold);
            for s in slice.iter_mut() {
                s.bold = !all_bold;
            }
            return;
        }
    }
    *default_bold = !*default_bold;
    for s in styles.iter_mut() {
        s.bold = *default_bold;
    }
}

/// Toggle italic on the selected character range (or all if no selection).
pub(crate) fn toggle_italic_range(styles: &mut [CharStyle], selection: Option<(usize, usize)>, default_italic: &mut bool) {
    if let Some((start, end)) = selection {
        let slice = styles.get_mut(start..end.min(styles.len())).unwrap_or(&mut []);
        if !slice.is_empty() {
            let all_italic = slice.iter().all(|s| s.italic);
            for s in slice.iter_mut() {
                s.italic = !all_italic;
            }
            return;
        }
    }
    *default_italic = !*default_italic;
    for s in styles.iter_mut() {
        s.italic = *default_italic;
    }
}

/// Toggle underline on the selected character range (or all if no selection).
pub(crate) fn toggle_underline_range(styles: &mut [CharStyle], selection: Option<(usize, usize)>) {
    if let Some((start, end)) = selection {
        let slice = styles.get_mut(start..end.min(styles.len())).unwrap_or(&mut []);
        if !slice.is_empty() {
            let all_underlined = slice.iter().all(|s| s.underline);
            for s in slice.iter_mut() {
                s.underline = !all_underlined;
            }
            return;
        }
    }
    let all_underlined = !styles.is_empty() && styles.iter().all(|s| s.underline);
    for s in styles.iter_mut() {
        s.underline = !all_underlined;
    }
}

/// Clear all formatting on the selected character range (or all if no selection).
pub(crate) fn clear_formatting_range(
    styles: &mut [CharStyle],
    selection: Option<(usize, usize)>,
    default_bold: &mut bool,
    default_italic: &mut bool,
) {
    if let Some((start, end)) = selection {
        let slice = styles.get_mut(start..end.min(styles.len())).unwrap_or(&mut []);
        for s in slice.iter_mut() {
            *s = CharStyle::default();
        }
        return;
    }
    *default_bold = false;
    *default_italic = false;
    for s in styles.iter_mut() {
        *s = CharStyle::default();
    }
}

/// Apply a color to the selected character range (or all if no selection).
pub(crate) fn apply_color_range(styles: &mut [CharStyle], selection: Option<(usize, usize)>, color: Option<Color32>) {
    if let Some((start, end)) = selection {
        let slice = styles.get_mut(start..end.min(styles.len())).unwrap_or(&mut []);
        for s in slice.iter_mut() {
            s.color = color;
        }
        return;
    }
    for s in styles.iter_mut() {
        s.color = color;
    }
}

/// Toggle superscript or subscript on the selected character range (or all of text).
pub(crate) fn apply_script_range(
    ctx: &egui::Context,
    field_id: egui::Id,
    text: &mut String,
    styles: &mut Vec<CharStyle>,
    selection: Option<(usize, usize)>,
    script: pdfcraft_engine::text_script::Script,
    fallback: (bool, bool),
) {
    let byte_range = selection.map(|(start, end)| {
        let b_start = text.char_indices().nth(start).map_or(text.len(), |(b, _)| b);
        let b_end = text.char_indices().nth(end).map_or(text.len(), |(b, _)| b);
        b_start..b_end
    });
    if let Some((new_text, new_range)) = pdfcraft_engine::text_script::toggle(text, byte_range.as_ref(), script) {
        *text = new_text;
        sync_char_styles(text, styles, fallback.0, fallback.1);
        let new_char_start = text.get(..new_range.start).map_or(0, |s| s.chars().count());
        let new_char_end = text.get(..new_range.end).map_or(0, |s| s.chars().count());
        reselect_range(ctx, field_id, new_char_start, new_char_end);
    }
}

/// Right-click context menu for comment text editors matching Acrobat's text options.
pub(crate) fn show_comment_text_context_menu(
    ui: &mut egui::Ui,
    field_id: egui::Id,
    text: &mut String,
    styles: &mut Vec<CharStyle>,
    undo: &mut Option<(String, Vec<CharStyle>)>,
    default_bold: &mut bool,
    default_italic: &mut bool,
) {
    ui.set_min_width(170.0);
    let sel_key = field_id.with("saved_selection");
    let mut selection = get_text_selection(ui.ctx(), field_id);
    if selection.is_none()
        && let Some((start, end)) = ui.data(|d| d.get_temp::<(usize, usize)>(sel_key))
    {
        let char_count = text.chars().count();
        if start < char_count && end <= char_count && start < end {
            reselect_range(ui.ctx(), field_id, start, end);
            selection = Some((start, end));
        }
    }
    let has_selection = selection.is_some();
    let can_undo = undo.is_some();

    if ui.add_enabled(can_undo, egui::Button::new(tl!("Undo Text Change"))).clicked() {
        if let Some((prev_text, prev_styles)) = undo.take() {
            *text = prev_text;
            *styles = prev_styles;
        }
        ui.data_mut(|d| d.remove::<(usize, usize)>(sel_key));
        ui.close();
    }
    ui.separator();

    if ui.add_enabled(has_selection, egui::Button::new(tl!("Cut Text"))).clicked() {
        if let Some((start, end)) = selection {
            *undo = Some((text.clone(), styles.clone()));
            let chars: Vec<char> = text.chars().collect();
            if start < chars.len() && end <= chars.len() && start < end {
                let cut_text: String = chars[start..end].iter().collect();
                ui.ctx().copy_text(cut_text);
                let mut new_chars = Vec::with_capacity(chars.len());
                new_chars.extend_from_slice(&chars[..start]);
                new_chars.extend_from_slice(&chars[end..]);
                *text = new_chars.into_iter().collect();
                if start < styles.len() && end <= styles.len() {
                    styles.drain(start..end);
                }
                reposition_cursor(ui.ctx(), field_id, start);
                ui.data_mut(|d| d.remove::<(usize, usize)>(sel_key));
            }
        }
        ui.close();
    }

    if ui.add_enabled(has_selection, egui::Button::new(tl!("Copy Text"))).clicked() {
        if let Some((start, end)) = selection {
            let chars: Vec<char> = text.chars().collect();
            if start < chars.len() && end <= chars.len() && start < end {
                let copy_text: String = chars[start..end].iter().collect();
                ui.ctx().copy_text(copy_text);
                reselect_range(ui.ctx(), field_id, start, end);
            }
        }
        ui.close();
    }

    let clip_text = crate::os_clipboard::with(|cb| cb.get_text()).ok().filter(|s| !s.is_empty());
    let can_paste = clip_text.is_some();
    if ui.add_enabled(can_paste, egui::Button::new(tl!("Paste Text"))).clicked() {
        if let Some(pasted) = clip_text {
            *undo = Some((text.clone(), styles.clone()));
            let pasted_chars: Vec<char> = pasted.chars().collect();
            let chars: Vec<char> = text.chars().collect();
            let (start, end) = selection.unwrap_or_else(|| {
                let pos = egui::TextEdit::load_state(ui.ctx(), field_id)
                    .and_then(|s| s.cursor.char_range())
                    .map_or(chars.len(), |r| r.primary.index.into());
                (pos.min(chars.len()), pos.min(chars.len()))
            });
            let inherit_style =
                styles.get(start).copied().unwrap_or(CharStyle { bold: *default_bold, italic: *default_italic, underline: false, color: None });
            let mut new_chars = Vec::with_capacity(chars.len() + pasted_chars.len());
            new_chars.extend_from_slice(&chars[..start]);
            new_chars.extend_from_slice(&pasted_chars);
            if end < chars.len() {
                new_chars.extend_from_slice(&chars[end..]);
            }
            *text = new_chars.into_iter().collect();
            if start <= styles.len() {
                let remove_end = end.min(styles.len());
                styles.drain(start..remove_end);
                let insert_styles = vec![inherit_style; pasted_chars.len()];
                styles.splice(start..start, insert_styles);
            }
            reposition_cursor(ui.ctx(), field_id, start + pasted_chars.len());
            ui.data_mut(|d| d.remove::<(usize, usize)>(sel_key));
        }
        ui.close();
    }

    if ui.add_enabled(has_selection, egui::Button::new(tl!("Delete Text"))).clicked() {
        if let Some((start, end)) = selection {
            *undo = Some((text.clone(), styles.clone()));
            let chars: Vec<char> = text.chars().collect();
            if start < chars.len() && end <= chars.len() && start < end {
                let mut new_chars = Vec::with_capacity(chars.len());
                new_chars.extend_from_slice(&chars[..start]);
                new_chars.extend_from_slice(&chars[end..]);
                *text = new_chars.into_iter().collect();
                if start < styles.len() && end <= styles.len() {
                    styles.drain(start..end);
                }
                reposition_cursor(ui.ctx(), field_id, start);
                ui.data_mut(|d| d.remove::<(usize, usize)>(sel_key));
            }
        }
        ui.close();
    }

    ui.separator();
    ui.add_enabled(false, egui::Button::new(tl!("Check Spelling")));
    ui.separator();

    ui.menu_button(tl!("Text Style"), |ui| {
        ui.set_min_width(170.0);
        if ui.button(tl!("Bold\tCtrl+B")).clicked() {
            *undo = Some((text.clone(), styles.clone()));
            toggle_bold_range(styles, selection, default_bold);
            if let Some((start, end)) = selection {
                reselect_range(ui.ctx(), field_id, start, end);
            }
            ui.close();
        }
        if ui.button(tl!("Italic\tCtrl+I")).clicked() {
            *undo = Some((text.clone(), styles.clone()));
            toggle_italic_range(styles, selection, default_italic);
            if let Some((start, end)) = selection {
                reselect_range(ui.ctx(), field_id, start, end);
            }
            ui.close();
        }
        if ui.button(tl!("Underline\tCtrl+U")).clicked() {
            *undo = Some((text.clone(), styles.clone()));
            toggle_underline_range(styles, selection);
            if let Some((start, end)) = selection {
                reselect_range(ui.ctx(), field_id, start, end);
            }
            ui.close();
        }
        if ui.button(tl!("Superscript\tShift+Ctrl+Plus")).clicked() {
            *undo = Some((text.clone(), styles.clone()));
            apply_script_range(
                ui.ctx(),
                field_id,
                text,
                styles,
                selection,
                pdfcraft_engine::text_script::Script::Super,
                (*default_bold, *default_italic),
            );
            ui.close();
        }
        if ui.button(tl!("Subscript\tCtrl+Plus")).clicked() {
            *undo = Some((text.clone(), styles.clone()));
            apply_script_range(
                ui.ctx(),
                field_id,
                text,
                styles,
                selection,
                pdfcraft_engine::text_script::Script::Sub,
                (*default_bold, *default_italic),
            );
            ui.close();
        }
        ui.separator();
        if ui.button(tl!("Clear Formatting")).clicked() {
            *undo = Some((text.clone(), styles.clone()));
            clear_formatting_range(styles, selection, default_bold, default_italic);
            if let Some((start, end)) = selection {
                reselect_range(ui.ctx(), field_id, start, end);
            }
            ui.close();
        }
    });

    ui.menu_button(tl!("Text Color"), |ui| {
        if let Some(rgb) = crate::comments::swatch_grid(ui, None) {
            *undo = Some((text.clone(), styles.clone()));
            let color = Color32::from_rgb((rgb[0] * 255.0) as u8, (rgb[1] * 255.0) as u8, (rgb[2] * 255.0) as u8);
            apply_color_range(styles, selection, Some(color));
            if let Some((start, end)) = selection {
                reselect_range(ui.ctx(), field_id, start, end);
            }
            ui.close();
        }
    });
}

/// Handle bold (Ctrl/Cmd+B) and italic (Ctrl/Cmd+I) shortcuts for comment editors.
pub(crate) fn format_keys(ui: &mut egui::Ui, field: &egui::Response, bold: &mut bool, italic: &mut bool) {
    if !field.has_focus() {
        return;
    }
    let mut toggle_bold = false;
    let mut toggle_italic = false;
    ui.input_mut(|input| {
        input.events.retain(|event| {
            if let egui::Event::Key { key, pressed: true, modifiers, .. } = event {
                let cmd_or_ctrl = (modifiers.command || modifiers.ctrl) && !modifiers.shift && !modifiers.alt;
                if *key == egui::Key::B && cmd_or_ctrl {
                    toggle_bold = true;
                    return false;
                }
                if *key == egui::Key::I && cmd_or_ctrl {
                    toggle_italic = true;
                    return false;
                }
            }
            true
        });
    });
    if toggle_bold {
        *bold = !*bold;
    }
    if toggle_italic {
        *italic = !*italic;
    }
}

/// Handle full keyboard formatting shortcuts (Ctrl+B, Ctrl+I, Ctrl+U, Shift+Ctrl+Plus, Ctrl+Plus)
/// for comment editors with selection awareness.
pub(crate) fn format_editor_keys(
    ui: &mut egui::Ui,
    field: &egui::Response,
    text: &mut String,
    styles: &mut Vec<CharStyle>,
    undo: &mut Option<(String, Vec<CharStyle>)>,
    bold: &mut bool,
    italic: &mut bool,
) {
    if !field.has_focus() {
        return;
    }
    let mut toggle_bold = false;
    let mut toggle_italic = false;
    let mut toggle_underline = false;
    let mut toggle_super = false;
    let mut toggle_sub = false;

    ui.input_mut(|input| {
        input.events.retain(|event| {
            if let egui::Event::Key { key, pressed: true, modifiers, .. } = event {
                let cmd_or_ctrl = (modifiers.command || modifiers.ctrl) && !modifiers.alt;
                if cmd_or_ctrl && !modifiers.shift {
                    if *key == egui::Key::B {
                        toggle_bold = true;
                        return false;
                    }
                    if *key == egui::Key::I {
                        toggle_italic = true;
                        return false;
                    }
                    if *key == egui::Key::U {
                        toggle_underline = true;
                        return false;
                    }
                    if *key == egui::Key::Plus || *key == egui::Key::Equals {
                        toggle_sub = true;
                        return false;
                    }
                }
                if cmd_or_ctrl && modifiers.shift && (*key == egui::Key::Plus || *key == egui::Key::Equals) {
                    toggle_super = true;
                    return false;
                }
            }
            true
        });
    });

    let selection = get_text_selection(ui.ctx(), field.id);
    if toggle_bold {
        *undo = Some((text.clone(), styles.clone()));
        toggle_bold_range(styles, selection, bold);
    }
    if toggle_italic {
        *undo = Some((text.clone(), styles.clone()));
        toggle_italic_range(styles, selection, italic);
    }
    if toggle_underline {
        *undo = Some((text.clone(), styles.clone()));
        toggle_underline_range(styles, selection);
    }
    if toggle_super {
        *undo = Some((text.clone(), styles.clone()));
        apply_script_range(ui.ctx(), field.id, text, styles, selection, pdfcraft_engine::text_script::Script::Super, (*bold, *italic));
    }
    if toggle_sub {
        *undo = Some((text.clone(), styles.clone()));
        apply_script_range(ui.ctx(), field.id, text, styles, selection, pdfcraft_engine::text_script::Script::Sub, (*bold, *italic));
    }
}

/// Render comment text in TextEdit with visual bold (semibold) and italic styling.
pub(crate) fn format_comment_galley(
    ui: &egui::Ui,
    text: &str,
    font_size: f32,
    wrap_width: f32,
    text_color: Color32,
    bold: bool,
    italic: bool,
) -> std::sync::Arc<egui::Galley> {
    format_comment_galley_styled(ui, text, font_size, wrap_width, text_color, &[], (bold, italic))
}

/// Render comment text in TextEdit with fine-grained per-character visual styling (bold, italic, underline, color).
pub(crate) fn format_comment_galley_styled(
    ui: &egui::Ui,
    text: &str,
    font_size: f32,
    wrap_width: f32,
    text_color: Color32,
    styles: &[CharStyle],
    fallback: (bool, bool),
) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = wrap_width;
    if text.is_empty() {
        return ui.painter().layout_job(job);
    }
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let st = styles.get(i).copied().unwrap_or(CharStyle { bold: fallback.0, italic: fallback.1, underline: false, color: None });
        let mut run = String::new();
        while i < chars.len() {
            let next_st = styles.get(i).copied().unwrap_or(CharStyle { bold: fallback.0, italic: fallback.1, underline: false, color: None });
            if next_st != st {
                break;
            }
            run.push(chars[i]);
            i += 1;
        }
        let font_id = if st.bold { theme::semibold(font_size) } else { theme::regular(font_size) };
        let color = st.color.unwrap_or(text_color);
        let underline = if st.underline { egui::Stroke::new(1.0, color) } else { egui::Stroke::NONE };
        job.append(&run, 0.0, egui::TextFormat { font_id, color, italics: st.italic, underline, ..Default::default() });
    }
    ui.painter().layout_job(job)
}

/// Convert comment text and character styles into PDF-compliant rich text XML (`/RC`).
pub(crate) fn styles_to_rc(text: &str, styles: &[CharStyle], default_bold: bool, default_italic: bool) -> String {
    let mut out = String::from(
        "<?xml version=\"1.0\"?><body xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:xfa=\"http://www.xfa.org/schema/xfa-data/1.0/\" xfa:spec=\"2.0.2\" style=\"font-size:12pt;text-align:left;color:#000000;font-weight:normal;font-style:normal;font-family:Helvetica;font-stretch:normal\">",
    );
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;");
    let mut char_idx = 0;
    for line in text.split('\n') {
        let line_chars: Vec<char> = line.chars().collect();
        out.push_str("<p dir=\"ltr\">");
        let mut i = 0;
        while i < line_chars.len() {
            let st =
                styles.get(char_idx + i).copied().unwrap_or(CharStyle { bold: default_bold, italic: default_italic, underline: false, color: None });
            let mut span_text = String::new();
            while i < line_chars.len() {
                let next_st = styles.get(char_idx + i).copied().unwrap_or(CharStyle {
                    bold: default_bold,
                    italic: default_italic,
                    underline: false,
                    color: None,
                });
                if next_st != st {
                    break;
                }
                span_text.push(line_chars[i]);
                i += 1;
            }
            let is_plain = !st.bold && !st.italic && !st.underline && st.color.is_none();
            let escaped = esc(&span_text);
            if is_plain {
                out.push_str(&escaped);
            } else {
                out.push_str("<span style=\"");
                if st.bold {
                    out.push_str("font-weight:bold;");
                }
                if st.italic {
                    out.push_str("font-style:italic;");
                }
                if st.underline {
                    out.push_str("text-decoration:underline;");
                }
                if let Some(c) = st.color {
                    out.push_str(&format!("color:#{:02X}{:02X}{:02X};", c.r(), c.g(), c.b()));
                }
                out.push_str("\">");
                out.push_str(&escaped);
                out.push_str("</span>");
            }
        }
        out.push_str("</p>");
        char_idx += line_chars.len() + 1;
    }
    out.push_str("</body>");
    out
}

/// Parse `/RC` XML rich text into per-character styles mapped to `text`.
pub(crate) fn rc_to_char_styles(rc: &str, text: &str) -> (Vec<CharStyle>, bool, bool) {
    let count = text.chars().count();
    if rc.trim().is_empty() {
        return (vec![CharStyle { bold: false, italic: false, underline: false, color: None }; count], false, false);
    }
    let mut default_style = CharStyle { bold: false, italic: false, underline: false, color: None };
    if let Some(body_start) = rc.find("<body")
        && let Some(tag_end) = rc[body_start..].find('>')
    {
        let body_tag = &rc[body_start..body_start + tag_end];
        if body_tag.contains("font-weight:bold") || body_tag.contains("font-weight: bold") || body_tag.contains("font-weight:700") {
            default_style.bold = true;
        }
        if body_tag.contains("font-style:italic") || body_tag.contains("font-style: italic") {
            default_style.italic = true;
        }
    }
    let mut style_stack = vec![default_style];
    let mut current_style = default_style;
    let mut parsed_chars = Vec::new();
    let mut parsed_styles = Vec::new();

    let mut rest = rc;
    while !rest.is_empty() {
        if let Some(tag_start) = rest.find('<') {
            let text_before = &rest[..tag_start];
            decode_xml_into(text_before, current_style, &mut parsed_chars, &mut parsed_styles);
            rest = &rest[tag_start..];
            if let Some(tag_end) = rest.find('>') {
                let tag = &rest[1..tag_end];
                rest = &rest[tag_end + 1..];
                if let Some(close_tag) = tag.strip_prefix('/') {
                    let name = close_tag.split_whitespace().next().unwrap_or_default().to_ascii_lowercase();
                    if matches!(name.as_str(), "span" | "b" | "strong" | "i" | "em" | "u" | "p" | "body") {
                        current_style = style_stack.pop().unwrap_or(default_style);
                    }
                } else {
                    let name = tag.split_whitespace().next().unwrap_or_default().to_ascii_lowercase();
                    let name = name.trim_end_matches('/');
                    style_stack.push(current_style);
                    if matches!(name, "b" | "strong") {
                        current_style.bold = true;
                    } else if matches!(name, "i" | "em") {
                        current_style.italic = true;
                    } else if name == "u" {
                        current_style.underline = true;
                    } else if matches!(name, "span" | "p" | "body")
                        && let Some(style_attr) = parse_style_attr(tag)
                    {
                        if style_attr.contains("font-weight:bold")
                            || style_attr.contains("font-weight: bold")
                            || style_attr.contains("font-weight:700")
                        {
                            current_style.bold = true;
                        } else if style_attr.contains("font-weight:normal") || style_attr.contains("font-weight: normal") {
                            current_style.bold = false;
                        }
                        if style_attr.contains("font-style:italic") || style_attr.contains("font-style: italic") {
                            current_style.italic = true;
                        } else if style_attr.contains("font-style:normal") || style_attr.contains("font-style: normal") {
                            current_style.italic = false;
                        }
                        if style_attr.contains("text-decoration:underline") || style_attr.contains("text-decoration: underline") {
                            current_style.underline = true;
                        }
                        if let Some(color) = parse_css_color(style_attr) {
                            current_style.color = Some(color);
                        }
                    }
                }
            } else {
                break;
            }
        } else {
            decode_xml_into(rest, current_style, &mut parsed_chars, &mut parsed_styles);
            break;
        }
    }

    let mut out_styles = Vec::with_capacity(count);
    let target_chars: Vec<char> = text.chars().collect();
    let mut p_idx = 0;
    for &ch in &target_chars {
        if ch == '\n' || ch == '\r' {
            out_styles.push(default_style);
            continue;
        }
        while p_idx < parsed_chars.len() && parsed_chars[p_idx].is_whitespace() && ch != parsed_chars[p_idx] {
            p_idx += 1;
        }
        if p_idx < parsed_chars.len() {
            out_styles.push(parsed_styles[p_idx]);
            p_idx += 1;
        } else {
            out_styles.push(default_style);
        }
    }
    while out_styles.len() < count {
        out_styles.push(default_style);
    }
    let all_bold = !out_styles.is_empty() && out_styles.iter().all(|s| s.bold);
    let all_italic = !out_styles.is_empty() && out_styles.iter().all(|s| s.italic);
    (out_styles, all_bold, all_italic)
}

fn parse_style_attr(tag: &str) -> Option<&str> {
    let lower = tag.to_ascii_lowercase();
    let idx = lower.find("style=")?;
    let rem = &tag[idx + 6..];
    let quote = rem.chars().next()?;
    if quote == '"' || quote == '\'' {
        let content = &rem[1..];
        let end = content.find(quote)?;
        Some(&content[..end])
    } else {
        None
    }
}

fn parse_css_color(style: &str) -> Option<Color32> {
    if let Some(idx) = style.find("color:#") {
        let hex = &style[idx + 7..];
        let hex = hex.split(|c: char| !c.is_ascii_hexdigit()).next()?;
        if hex.len() >= 6 {
            let r = u8::from_str_radix(hex.get(0..2)?, 16).ok()?;
            let g = u8::from_str_radix(hex.get(2..4)?, 16).ok()?;
            let b = u8::from_str_radix(hex.get(4..6)?, 16).ok()?;
            return Some(Color32::from_rgb(r, g, b));
        }
    }
    None
}

fn decode_xml_into(s: &str, style: CharStyle, chars: &mut Vec<char>, styles: &mut Vec<CharStyle>) {
    let mut rest = s;
    while !rest.is_empty() {
        if let Some(amp) = rest.find('&') {
            for ch in rest.get(..amp).unwrap_or_default().chars() {
                chars.push(ch);
                styles.push(style);
            }
            rest = rest.get(amp..).unwrap_or_default();
            if let Some(semi) = rest.find(';') {
                let ent = rest.get(1..semi).unwrap_or_default();
                let decoded = match ent {
                    "amp" => Some('&'),
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "quot" => Some('"'),
                    "apos" => Some('\''),
                    _ if ent.starts_with('#') => {
                        let num = ent.get(1..).unwrap_or_default();
                        if let Some(hex) = num.strip_prefix('x').or_else(|| num.strip_prefix('X')) {
                            u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
                        } else {
                            num.parse::<u32>().ok().and_then(char::from_u32)
                        }
                    }
                    _ => None,
                };
                if let Some(c) = decoded {
                    chars.push(c);
                    styles.push(style);
                }
                rest = rest.get(semi + 1..).unwrap_or_default();
            } else {
                for ch in rest.chars() {
                    chars.push(ch);
                    styles.push(style);
                }
                break;
            }
        } else {
            for ch in rest.chars() {
                chars.push(ch);
                styles.push(style);
            }
            break;
        }
    }
}
