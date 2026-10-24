//! The source editor's language support and its large-file viewer: highlighting that follows Lua's
//! long strings and comments, INI and SVG, diagnostics from the game's own compilers (the Lua a
//! skin runs, `ddi_lua`, and the SVG subset, `ddi_noteskin::svg`), and a bounded read-only view for
//! generated files too large to lay out at once.
use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontId};

/// What a file is, for highlighting and diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Language {
    Lua,
    Ini,
    Svg,
    Plain,
}

impl Language {
    pub fn of(path: &std::path::Path) -> Language {
        match path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("lua") => Language::Lua,
            Some("ini" | "redir") => Language::Ini,
            Some("svg" | "xml") => Language::Svg,
            _ => Language::Plain,
        }
    }
}

/// The largest text edited in place; larger ones open in [`LargeView`].
pub const MAX_EDITED: usize = 128 << 10;
/// The longest line edited in place (generated single-line SVGs are far longer).
pub const MAX_EDITED_LINE: usize = 4096;

/// Whether `text` is laid out whole for editing; otherwise it is shown in [`LargeView`].
pub fn editable(text: &str) -> bool {
    text.len() <= MAX_EDITED && text.split('\n').all(|l| l.len() <= MAX_EDITED_LINE)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Text,
    Keyword,
    Name,
    String,
    Number,
    Comment,
    Punct,
}

/// The colours tokens are drawn in, from the theme's visuals.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Colors {
    text: Color32,
    keyword: Color32,
    name: Color32,
    string: Color32,
    number: Color32,
    comment: Color32,
    punct: Color32,
}

impl Colors {
    pub fn from_visuals(v: &egui::Visuals) -> Colors {
        let dark = v.dark_mode;
        Colors {
            text: v.text_color(),
            keyword: v.hyperlink_color,
            name: if dark {
                Color32::from_rgb(0x8c, 0xc8, 0xff)
            } else {
                Color32::from_rgb(0x1f, 0x5f, 0xa8)
            },
            string: if dark {
                Color32::from_rgb(0x9f, 0xd4, 0x9a)
            } else {
                Color32::from_rgb(0x2f, 0x7d, 0x32)
            },
            number: if dark {
                Color32::from_rgb(0xff, 0xd1, 0x66)
            } else {
                Color32::from_rgb(0xa6, 0x6f, 0x00)
            },
            comment: v.weak_text_color(),
            punct: v.text_color().gamma_multiply(0.75),
        }
    }

    fn of(&self, kind: Kind) -> Color32 {
        match kind {
            Kind::Text => self.text,
            Kind::Keyword => self.keyword,
            Kind::Name => self.name,
            Kind::String => self.string,
            Kind::Number => self.number,
            Kind::Comment => self.comment,
            Kind::Punct => self.punct,
        }
    }
}

const LUA_KEYWORDS: &[&str] = &[
    "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "if", "in", "local",
    "nil", "not", "or", "repeat", "return", "then", "true", "until", "while",
];

/// Names a skin's Lua reads from the game (StepMania's, and our extensions), drawn apart.
const LUA_NAMES: &[&str] = &[
    "cmd",
    "Def",
    "NOTESKIN",
    "Var",
    "GAMESTATE",
    "SCREENMAN",
    "THEME",
    "Sprite",
    "ActorFrame",
    "LoadActor",
    "self",
    "string",
    "math",
    "table",
    "pairs",
    "ipairs",
    "type",
    "tostring",
    "tonumber",
    "print",
    "pcall",
    "error",
    "assert",
    "select",
    "unpack",
    "setmetatable",
];

/// The length of a long bracket's opening at the start of `s` (`[[`, `[==[`), and its level.
fn long_open(s: &[u8]) -> Option<(usize, usize)> {
    if s.first() != Some(&b'[') {
        return None;
    }
    let level = s[1..].iter().take_while(|&&b| b == b'=').count();
    (s.get(1 + level) == Some(&b'[')).then_some((level + 2, level))
}

/// Where the long bracket of `level` closes in `s` (the end of its `]==]`), or the end.
fn long_close(s: &[u8], level: usize) -> usize {
    let mut i = 0;
    while i < s.len() {
        if s[i] == b']'
            && s[i + 1..].iter().take(level).all(|&b| b == b'=')
            && s.len() > i + 1 + level
            && s[i + 1 + level] == b']'
        {
            return i + level + 2;
        }
        i += 1;
    }
    s.len()
}

/// A quoted string's length from its opening quote: to the closing quote or the line's end.
fn quoted(s: &[u8]) -> usize {
    let quote = s[0];
    let mut i = 1;
    while i < s.len() {
        match s[i] {
            b'\\' => i += 2,
            b'\n' => return i,
            c if c == quote => return i + 1,
            _ => i += 1,
        }
    }
    s.len().min(i)
}

fn tokens(text: &str, language: Language) -> Vec<(std::ops::Range<usize>, Kind)> {
    let s = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    let ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b >= 0x80;
    let mut line_start = true;
    while i < s.len() {
        let rest = &s[i..];
        let (len, kind) = match language {
            Language::Lua => {
                if rest.starts_with(b"--") {
                    match long_open(&rest[2..]) {
                        Some((open, level)) => (
                            2 + open + long_close(&rest[2 + open..], level),
                            Kind::Comment,
                        ),
                        None => (
                            rest.iter().position(|&b| b == b'\n').unwrap_or(rest.len()),
                            Kind::Comment,
                        ),
                    }
                } else if let Some((open, level)) = long_open(rest) {
                    (open + long_close(&rest[open..], level), Kind::String)
                } else if rest[0] == b'"' || rest[0] == b'\'' {
                    (quoted(rest), Kind::String)
                } else if rest[0].is_ascii_digit()
                    || (rest[0] == b'.' && rest.get(1).is_some_and(u8::is_ascii_digit))
                {
                    let mut n = 1;
                    while n < rest.len()
                        && (rest[n].is_ascii_alphanumeric()
                            || rest[n] == b'.'
                            || ((rest[n] == b'-' || rest[n] == b'+')
                                && matches!(rest[n - 1], b'e' | b'E' | b'p' | b'P')))
                    {
                        n += 1;
                    }
                    (n, Kind::Number)
                } else if ident(rest[0]) {
                    let n = rest.iter().position(|&b| !ident(b)).unwrap_or(rest.len());
                    let word = &text[i..i + n];
                    let kind = if LUA_KEYWORDS.contains(&word) {
                        Kind::Keyword
                    } else if LUA_NAMES.contains(&word) {
                        Kind::Name
                    } else {
                        Kind::Text
                    };
                    (n, kind)
                } else if rest[0].is_ascii_punctuation() {
                    (1, Kind::Punct)
                } else {
                    (1, Kind::Text)
                }
            }
            Language::Ini => {
                let line = rest.iter().position(|&b| b == b'\n').unwrap_or(rest.len());
                if line_start && matches!(rest[0], b';' | b'#') {
                    (line, Kind::Comment)
                } else if line_start && rest[0] == b'[' {
                    (line, Kind::Keyword)
                } else if line_start && rest[0] != b'\n' && !rest[0].is_ascii_whitespace() {
                    let key = rest[..line].iter().position(|&b| b == b'=').unwrap_or(line);
                    (key.max(1), Kind::Name)
                } else if rest[0].is_ascii_digit() {
                    let n = rest
                        .iter()
                        .position(|&b| !(b.is_ascii_digit() || b == b'.'))
                        .unwrap_or(rest.len());
                    (n, Kind::Number)
                } else {
                    (
                        1,
                        if rest[0] == b'=' {
                            Kind::Punct
                        } else {
                            Kind::Text
                        },
                    )
                }
            }
            Language::Svg => {
                if rest.starts_with(b"<!--") {
                    let end = text[i..].find("-->").map_or(rest.len(), |e| e + 3);
                    (end, Kind::Comment)
                } else if rest[0] == b'<' {
                    let n = 1 + rest[1..]
                        .iter()
                        .position(|&b| !(ident(b) || matches!(b, b'/' | b'?' | b':' | b'-')))
                        .unwrap_or(rest.len() - 1);
                    (n, Kind::Keyword)
                } else if rest[0] == b'"' || rest[0] == b'\'' {
                    (quoted(rest), Kind::String)
                } else if ident(rest[0]) {
                    let n = rest
                        .iter()
                        .position(|&b| !(ident(b) || matches!(b, b':' | b'-')))
                        .unwrap_or(rest.len());
                    let attr = rest.get(n) == Some(&b'=');
                    (n, if attr { Kind::Name } else { Kind::Text })
                } else if matches!(rest[0], b'>' | b'/' | b'=') {
                    (1, Kind::Keyword)
                } else {
                    (1, Kind::Text)
                }
            }
            Language::Plain => (rest.len(), Kind::Text),
        };
        let len = len.max(1).min(rest.len());
        // Tokens end on character boundaries.
        let mut end = i + len;
        while !text.is_char_boundary(end) {
            end += 1;
        }
        line_start = s[end - 1] == b'\n';
        out.push((i..end, kind));
        i = end;
    }
    out
}

/// `text` highlighted as `language`, in monospace at `size` points.
pub fn highlight(text: &str, language: Language, colors: &Colors, size: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    let font = FontId::monospace(size);
    let mut last: Option<(std::ops::Range<usize>, Kind)> = None;
    let flush = |job: &mut LayoutJob, range: std::ops::Range<usize>, kind: Kind| {
        job.append(
            &text[range],
            0.0,
            TextFormat {
                font_id: font.clone(),
                color: colors.of(kind),
                ..Default::default()
            },
        );
    };
    // Neighbouring tokens of one colour make one section: fewer sections, faster layout.
    for (range, kind) in tokens(text, language) {
        match &mut last {
            Some((r, k)) if colors.of(*k) == colors.of(kind) => r.end = range.end,
            _ => {
                if let Some((r, k)) = last.take() {
                    flush(&mut job, r, k);
                }
                last = Some((range, kind));
            }
        }
    }
    if let Some((r, k)) = last {
        flush(&mut job, r, k);
    }
    job
}

/// A problem the game would report loading a file: its line (1-based, when known) and message.
#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostic {
    pub line: Option<usize>,
    pub message: String,
}

/// What the game says of `text`: a Lua chunk compiled as a skin's is (Lua 5.1 with StepMania's
/// `cmd`, within the game's limits; nothing runs), or an SVG checked against the game's subset.
pub fn diagnose(text: &str, language: Language, name: &str) -> Option<Diagnostic> {
    match language {
        Language::Lua => {
            let mut vm = ddi_lua::Vm::new(ddi_lua::Settings::default());
            let failure = vm.load(&format!("@{name}"), text.as_bytes()).err()?;
            let message = failure.to_string();
            // "name:line: message"
            let after = message.strip_prefix(name).and_then(|m| m.strip_prefix(':'));
            let line = after.and_then(|m| m.split(':').next()?.trim().parse().ok());
            let message = match (line, after) {
                (Some(_), Some(m)) => m.split_once(':').map_or(m, |(_, m)| m).trim().to_string(),
                _ => message,
            };
            Some(Diagnostic { line, message })
        }
        Language::Svg => ddi_noteskin::svg::check_svg(text)
            .err()
            .map(|message| Diagnostic {
                line: None,
                message,
            }),
        _ => None,
    }
}

/// The longest part of a line [`LargeView`] lays out.
pub const SHOWN_LINE: usize = 2000;

/// A read-only view of a large text: only the visible lines are laid out, each to at most
/// [`SHOWN_LINE`] bytes, so a generated multi-megabyte, single-line SVG costs what a screenful
/// does. The line index is built once per text.
#[derive(Default)]
pub struct LargeView {
    key: u64,
    lines: Vec<std::ops::Range<usize>>,
    /// Lines broken between elements (an SVG's `><`), not only at newlines.
    by_element: bool,
}

impl LargeView {
    /// Shows `text`; an SVG's lines are broken between its elements too, so a generated
    /// single-line file reads one element a line.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        id: impl std::hash::Hash + std::fmt::Debug,
        text: &str,
        language: Language,
    ) {
        let by_element = language == Language::Svg;
        let key = {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            text.len().hash(&mut h);
            text.as_bytes()[..text.len().min(4096)].hash(&mut h);
            h.finish()
        };
        if key != self.key || self.lines.is_empty() || by_element != self.by_element {
            self.key = key;
            self.by_element = by_element;
            self.lines.clear();
            let bytes = text.as_bytes();
            let mut start = 0;
            for (i, &b) in bytes.iter().enumerate() {
                if b == b'\n' {
                    self.lines.push(start..i);
                    start = i + 1;
                } else if by_element && b == b'>' && bytes.get(i + 1) == Some(&b'<') {
                    self.lines.push(start..i + 1);
                    start = i + 1;
                }
            }
            self.lines.push(start..text.len());
        }
        let longest = self.lines.iter().map(|r| r.len()).max().unwrap_or(0);
        ui.small(format!(
            "{} KiB · {} {} · longest {} characters: shown read-only, lines cut at {SHOWN_LINE}. Edit generated files with their generator or MCP write_text.",
            text.len() / 1024,
            self.lines.len(),
            if by_element { "elements" } else { "lines" },
            longest
        ));
        let font = FontId::monospace(12.0);
        let row = ui.fonts_mut(|f| f.row_height(&font));
        let color = ui.visuals().text_color();
        egui::ScrollArea::both()
            .id_salt(("large-view", id))
            .auto_shrink(false)
            .show_rows(ui, row, self.lines.len(), |ui, range| {
                for n in range {
                    let r = self.lines[n].clone();
                    let mut end = r.start + r.len().min(SHOWN_LINE);
                    while !text.is_char_boundary(end) {
                        end -= 1;
                    }
                    let cut = end < r.end;
                    let line = format!(
                        "{:>6}  {}{}",
                        n + 1,
                        &text[r.start..end],
                        if cut { " …" } else { "" }
                    );
                    ui.add(
                        egui::Label::new(egui::RichText::new(line).font(font.clone()).color(color))
                            .extend(),
                    );
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(text: &str, language: Language) -> Vec<(&str, Kind)> {
        tokens(text, language)
            .into_iter()
            .map(|(r, k)| (&text[r], k))
            .filter(|(t, _)| !t.trim().is_empty())
            .collect()
    }

    #[test]
    fn lua_long_strings_and_comments_span_lines() {
        let src = "local a = [[x\n-- not a comment\n]] --[==[ one\ntwo ]==] return cmd(zoom,2)";
        let k = kinds(src, Language::Lua);
        assert_eq!(k[0], ("local", Kind::Keyword));
        assert!(k.contains(&("[[x\n-- not a comment\n]]", Kind::String)));
        assert!(k.contains(&("--[==[ one\ntwo ]==]", Kind::Comment)));
        assert!(k.contains(&("return", Kind::Keyword)));
        assert!(k.contains(&("cmd", Kind::Name)));
        assert!(k.contains(&("2", Kind::Number)));
        // Every byte is in one token.
        let total: usize = tokens(src, Language::Lua)
            .iter()
            .map(|(r, _)| r.len())
            .sum();
        assert_eq!(total, src.len());
    }

    #[test]
    fn ini_and_svg() {
        let k = kinds(
            "[Global]\nFallbackNoteSkin=default\n; note\n",
            Language::Ini,
        );
        assert_eq!(k[0], ("[Global]", Kind::Keyword));
        assert_eq!(k[1], ("FallbackNoteSkin", Kind::Name));
        assert!(k.contains(&("; note", Kind::Comment)));
        let k = kinds(r#"<svg width="4"><!-- c --></svg>"#, Language::Svg);
        assert_eq!(k[0], ("<svg", Kind::Keyword));
        assert_eq!(k[1], ("width", Kind::Name));
        assert!(k.contains(&("\"4\"", Kind::String)));
        assert!(k.contains(&("<!-- c -->", Kind::Comment)));
    }

    #[test]
    fn diagnostics_are_the_games() {
        let d = diagnose("local x = 1\nreturn x +\n", Language::Lua, "NoteSkin.lua").unwrap();
        assert_eq!(d.line, Some(3), "{d:?}");
        assert!(
            diagnose(
                "return Def.Actor{ InitCommand=cmd(zoom,2) }",
                Language::Lua,
                "a.lua"
            )
            .is_none()
        );
        assert!(
            diagnose(
                "<svg width=\"1\" height=\"1\"><text/></svg>",
                Language::Svg,
                "a.svg"
            )
            .is_some()
        );
        assert!(diagnose("[a]\nb=c", Language::Ini, "metrics.ini").is_none());
    }

    #[test]
    fn large_texts_are_not_edited_in_place() {
        assert!(editable("local a = 1\n"));
        assert!(!editable(&"x".repeat(MAX_EDITED_LINE + 1)));
        assert!(!editable(&"short line\n".repeat(MAX_EDITED / 10)));
    }
}
