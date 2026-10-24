//! The native source editor: tabs over a skin's scripts, metrics and SVG text, highlighted with
//! the game's diagnostics (`code.rs`); egui owns each file's independent edit/undo state. Large
//! generated files open read-only in a bounded view rather than being laid out whole.
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Default)]
pub struct Editor {
    pub tabs: Vec<PathBuf>,
    pub active: Option<PathBuf>,
    pub edits: HashMap<PathBuf, String>,
    buffers: HashMap<PathBuf, String>,
    originals: HashMap<PathBuf, String>,
    pub changed: Option<Instant>,
    pub visible: bool,
    pub read_only: HashSet<PathBuf>,
    namespace: String,
    find: String,
    find_open: bool,
    find_focus: bool,
    find_at: HashMap<PathBuf, usize>,
    scroll_to: Option<usize>,
    layouts: Layouts,
    pub diagnostics: Diagnostics,
    large: crate::code::LargeView,
}

impl Editor {
    pub fn for_project(path: &Path) -> Self {
        Self {
            namespace: crate::workspace::key(path),
            ..Default::default()
        }
    }

    pub fn open(&mut self, path: &Path) -> Result<(), String> {
        if !self.buffers.contains_key(path) {
            let original = crate::files::read_text(path)?;
            let text = self.edits.get(path).unwrap_or(&original).clone();
            self.buffers.insert(path.to_path_buf(), text);
            self.originals.insert(path.to_path_buf(), original);
        }
        if !self.tabs.iter().any(|p| p == path) {
            self.tabs.push(path.to_path_buf());
        }
        self.active = Some(path.to_path_buf());
        self.visible = true;
        Ok(())
    }

    pub fn set(&mut self, path: &Path, text: String) -> Result<(), String> {
        if text.len() > 1 << 20 {
            return Err("Text exceeds 1 MiB".into());
        }
        if self.originals.get(path) == Some(&text) {
            self.edits.remove(path);
        } else {
            self.edits.insert(path.to_path_buf(), text.clone());
        }
        self.buffers.insert(path.to_path_buf(), text);
        self.changed = Some(Instant::now());
        Ok(())
    }

    pub fn show(&mut self, ctx: &egui::Context, error: Option<&str>) -> bool {
        if !self.visible {
            return false;
        }
        let mut visible = self.visible;
        let mut edited = None;
        egui::Window::new("Noteskin scripts and text")
            .id(egui::Id::new("noteskin-text-editor"))
            .open(&mut visible)
            .default_pos(egui::pos2(90.0, 100.0))
            .default_size(egui::vec2(680.0, 520.0))
            .show(ctx, |ui| {
                egui::ScrollArea::horizontal().id_salt("script-tabs").show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let mut close = None;
                        for path in &self.tabs {
                            let name = path.file_name().unwrap_or_default().to_string_lossy();
                            let label = format!("{}{}", name, if self.edits.contains_key(path) { " •" } else { "" });
                            if ui.selectable_label(self.active.as_ref() == Some(path), label).on_hover_text(path.display().to_string()).clicked() { self.active = Some(path.clone()); }
                            if ui.small_button("×").on_hover_text("Close tab; project keeps edits").clicked() { close = Some(path.clone()); }
                        }
                        if let Some(path) = close {
                            self.tabs.retain(|p| p != &path);
                            if self.active.as_ref() == Some(&path) { self.active = self.tabs.last().cloned(); }
                        }
                    });
                });
                ui.separator();
                if let Some(error) = error { ui.colored_label(ui.visuals().error_fg_color, format!("Last working preview retained: {error}")); }
                let Some(path) = self.active.clone() else {
                    ui.label("Open a script, metrics file or SVG source from Noteskin Files.");
                    return;
                };
                let language = crate::code::Language::of(&path);
                let read_only = self.read_only.contains(&path);
                ui.small(path.display().to_string());
                if read_only { ui.small("This SVG is managed by the vector editor. Use SVG source in the file browser to switch to text editing."); }
                let Some(text) = self.buffers.get_mut(&path) else { return };
                if !crate::code::editable(text) {
                    self.large.show(ui, (&self.namespace, &path), text, language);
                    return;
                }
                // Find (⌘F), next with Enter.
                if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::F)) { self.find_open = true; self.find_focus = true; }
                let mut jump = None;
                if self.find_open {
                    ui.horizontal(|ui| {
                        let field = ui.add(egui::TextEdit::singleline(&mut self.find).hint_text("Find").desired_width(220.0));
                        if std::mem::take(&mut self.find_focus) { field.request_focus(); }
                        let next = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        let matches = if self.find.is_empty() { 0 } else { text.matches(self.find.as_str()).count() };
                        ui.small(format!("{matches} found"));
                        if (ui.small_button("Next").clicked() || next) && matches > 0 {
                            let from = self.find_at.get(&path).copied().unwrap_or(0);
                            let at = text[from.min(text.len())..].find(self.find.as_str()).map(|i| i + from).or_else(|| text.find(self.find.as_str()));
                            if let Some(at) = at {
                                self.find_at.insert(path.clone(), at + self.find.len());
                                jump = Some((at, at + self.find.len()));
                                if next { field.request_focus(); }
                            }
                        }
                        if ui.small_button("×").clicked() { self.find_open = false; }
                    });
                }
                let colors = crate::code::Colors::from_visuals(ui.visuals());
                let diagnostic = self.diagnostics.diagnose(&path, text, language);
                let lines = text.split('\n').count();
                let font = egui::FontId::monospace(12.5);
                let cache = &mut self.layouts;
                let mut layouter = |ui: &egui::Ui, value: &dyn egui::TextBuffer, _width: f32| {
                    let job = cache.job(value.as_str(), language, &colors, font.size);
                    ui.fonts_mut(|f| f.layout_job(job))
                };
                let id = egui::Id::new(("source", &self.namespace, &path));
                if let Some((a, b)) = jump {
                    let (a, b) = (text[..a].chars().count(), text[..b].chars().count());
                    let mut state = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
                    state.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(a), egui::text::CCursor::new(b))));
                    state.store(ctx, id);
                    self.scroll_to = Some(b);
                }
                let scroll_to = self.scroll_to.take();
                let output = egui::ScrollArea::both().id_salt(("source-scroll", &path)).max_height(ui.available_height() - 28.0).show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        // Line numbers, the error's in the error colour.
                        let mut gutter = egui::text::LayoutJob::default();
                        let weak = ui.visuals().weak_text_color();
                        let bad = ui.visuals().error_fg_color;
                        let width = lines.to_string().len().max(2);
                        for n in 1..=lines {
                            let color = if diagnostic.as_ref().and_then(|d| d.line) == Some(n) { bad } else { weak };
                            gutter.append(&format!("{n:>width$}\n"), 0.0, egui::text::TextFormat { font_id: font.clone(), color, ..Default::default() });
                        }
                        ui.add(egui::Label::new(gutter).selectable(false));
                        egui::TextEdit::multiline(text).id(id).code_editor().interactive(!read_only).desired_width(f32::INFINITY).desired_rows(18).char_limit(1 << 20).layouter(&mut layouter).show(ui)
                    }).inner
                }).inner;
                if let Some(at) = scroll_to {
                    let cursor = output.galley.pos_from_cursor(egui::text::CCursor::new(at));
                    ui.scroll_to_rect(cursor.translate(output.galley_pos.to_vec2()), Some(egui::Align::Center));
                }
                let response = output.response;
                let cursor = output.cursor_range.map(|r| usize::from(r.primary.index)).unwrap_or(0);
                let line = text.chars().take(cursor).filter(|c| *c == '\n').count() + 1;
                ui.horizontal(|ui| {
                    ui.small(format!("Line {line} · {lines} lines · ⌘F find · ⌘Z / ⇧⌘Z · reloads after typing pauses"));
                    if let Some(d) = &diagnostic {
                        let at = d.line.map(|l| format!("line {l}: ")).unwrap_or_default();
                        ui.colored_label(ui.visuals().error_fg_color, format!("{at}{}", d.message));
                    }
                });
                if response.changed() { edited = Some((path.clone(), text.clone())); }
            });
        self.visible = visible;
        if let Some((path, text)) = edited {
            return self.set(&path, text).is_ok();
        }
        false
    }
}

/// Highlighted layouts, kept while their text stays the same (laid out once, not every frame).
#[derive(Default)]
pub struct Layouts {
    last: Option<(u64, crate::code::Language, egui::text::LayoutJob)>,
}

impl Layouts {
    fn job(
        &mut self,
        text: &str,
        language: crate::code::Language,
        colors: &crate::code::Colors,
        size: f32,
    ) -> egui::text::LayoutJob {
        let key = hash((text, format!("{colors:?}"), size.to_bits()));
        match &self.last {
            Some((k, l, job)) if *k == key && *l == language => job.clone(),
            _ => {
                let job = crate::code::highlight(text, language, colors, size);
                self.last = Some((key, language, job.clone()));
                job
            }
        }
    }
}

/// The game's diagnostics of each file's current text, computed when it changes.
#[derive(Default)]
pub struct Diagnostics {
    by_path: HashMap<PathBuf, (u64, Option<crate::code::Diagnostic>)>,
}

impl Diagnostics {
    pub fn diagnose(
        &mut self,
        path: &Path,
        text: &str,
        language: crate::code::Language,
    ) -> Option<crate::code::Diagnostic> {
        let key = hash(text);
        if let Some((k, d)) = self.by_path.get(path)
            && *k == key
        {
            return d.clone();
        }
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let d = crate::code::diagnose(text, language, &name);
        self.by_path.insert(path.to_path_buf(), (key, d.clone()));
        d
    }
}

fn hash(value: impl std::hash::Hash) -> u64 {
    use std::hash::Hasher;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut h);
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A generated single-line SVG atlas near the 1 MiB limit, open in several tabs, used to make
    /// egui lay out millions of glyphs and ask for a 141 MB staging buffer (SC-01). It opens in
    /// the bounded view, and a small script beside it still edits in place.
    #[test]
    fn large_single_line_svgs_open_safely_in_several_tabs() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/scratch/large-text");
        std::fs::create_dir_all(&dir).unwrap();
        let mut editor = Editor::default();
        let cell = r##"<path d="M0 0h4v4z" fill="#ff6ea0"/>"##;
        for n in 0..3 {
            let path = dir.join(format!("atlas{n}.svg"));
            let mut svg =
                String::from(r#"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64">"#);
            while svg.len() < 950_000 {
                svg.push_str(cell);
            }
            svg.push_str("</svg>");
            std::fs::write(&path, &svg).unwrap();
            editor.open(&path).unwrap();
        }
        let script = dir.join("NoteSkin.lua");
        std::fs::write(
            &script,
            "-- Dito's factory\nlocal t = Def.ActorFrame{\n  --[[ a long\n  comment ]]\n  Def.Sprite{ Texture = [[_down tap note]], InitCommand = cmd(zoom,0.5) },\n}\nreturn t +\n",
        )
        .unwrap();
        editor.open(&script).unwrap();
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1000.0, 700.0))
            .wgpu()
            .build_ui_state(
                |ui, editor: &mut Editor| {
                    editor.show(ui.ctx(), None);
                },
                editor,
            );
        for n in 0..3 {
            h.state_mut().active = Some(dir.join(format!("atlas{n}.svg")));
            h.run_steps(2);
            h.render().unwrap().save(dir.join("atlas.png")).unwrap();
        }
        h.state_mut().active = Some(script.clone());
        h.run_steps(2);
        h.render().unwrap().save(dir.join("script.png")).unwrap();
        let d = h.state_mut().diagnostics.diagnose(
            &script,
            &std::fs::read_to_string(&script).unwrap(),
            crate::code::Language::Lua,
        );
        assert_eq!(d.and_then(|d| d.line), Some(8));
        assert!(crate::code::editable(&h.state().buffers[&script]));
    }
}
