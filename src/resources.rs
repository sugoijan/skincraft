//! The project's own files (SC-02): texts created and files imported into its skin's folder,
//! never written into the source folder (`project::Added`), and drafts discarded. Each change is
//! validated by reloading the preview through the game's loader; a change the loader refuses is
//! undone, keeping the last working preview. [`Panel::apply`] takes several at once, as one
//! revision with one reload (SC-03).
use super::*;

/// The largest file imported into a project.
pub const MAX_IMPORT: u64 = 16 << 20;

/// One change of a [`Panel::apply`] transaction.
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Change {
    /// A text draft of a project text file (an existing one or one the project added).
    Write {
        path: String,
        text: String,
        /// The draft's [`content_hash`] the change was made against: refused if it moved on.
        #[serde(default)]
        expect_hash: Option<String>,
    },
    /// A new text file in the skin's folder (or one overriding an inherited file).
    Create { path: String, text: String },
    /// A file copied into the skin's folder from `source` (an image, a model, a text).
    Import {
        source: String,
        path: String,
        /// Replace a file the project added before at `path`.
        #[serde(default)]
        replace: bool,
    },
    /// A file the project added, removed (an inherited file it overrode shows again); a file of
    /// the skin's own folder, left out of the preview and exports (the folder untouched).
    Remove { path: String },
    /// A file of the skin's own folder that was left out, back in.
    Restore { path: String },
    /// A source file's draft dropped: back to the file.
    Discard { path: String },
}

/// What a text is, for preconditions: FNV-1a of its bytes, 16 hex digits.
pub fn content_hash(text: &str) -> String {
    ddi_noteskin::report::hash(text.as_bytes())
}

/// What the project's state was before a transaction, to undo it.
struct Before {
    edits: HashMap<PathBuf, String>,
    added: project::AddedFiles,
    files: Vec<files::File>,
    tabs: Vec<PathBuf>,
    active: Option<PathBuf>,
}

impl Panel {
    /// Where a project file's bytes are: an imported file's copy, else the file itself.
    pub(super) fn readable(&self, path: &Path) -> PathBuf {
        match self.added.get(path) {
            Some(project::Added::File(copy)) => copy.clone(),
            _ => path.to_path_buf(),
        }
    }

    /// The imported files' copies, by the path each has in the skin's folder.
    pub(super) fn copies(&self) -> HashMap<PathBuf, PathBuf> {
        self.added
            .iter()
            .filter_map(|(path, file)| match file {
                project::Added::File(copy) => Some((path.clone(), copy.clone())),
                project::Added::Text | project::Added::Excluded => None,
            })
            .collect()
    }

    /// `path` (relative to the skin's folder, or absolute inside it) in the selected skin's folder.
    pub(super) fn own_path(&self, path: &str) -> Result<PathBuf, String> {
        let base = self
            .project
            .as_ref()
            .and_then(|p| p.skins.first())
            .map(|s| s.dir.clone())
            .ok_or("Load a skin first")?;
        let given = Path::new(path);
        let rel = if given.is_absolute() {
            given
                .strip_prefix(&base)
                .map_err(|_| format!("{path} is not in the skin's folder ({})", base.display()))?
                .to_path_buf()
        } else {
            given.to_path_buf()
        };
        if rel.as_os_str().is_empty()
            || rel
                .components()
                .any(|c| !matches!(c, std::path::Component::Normal(_)))
        {
            return Err(format!(
                "{path}: a plain path inside the skin's folder is needed"
            ));
        }
        let name = rel.to_string_lossy();
        if name.len() > 255 || name.contains(['\0', '\\']) {
            return Err(format!("{path}: unsupported file name"));
        }
        Ok(base.join(rel))
    }

    fn before(&self) -> Before {
        Before {
            edits: self.editor.edits.clone(),
            added: self.added.clone(),
            files: self.files.clone(),
            tabs: self.editor.tabs.clone(),
            active: self.editor.active.clone(),
        }
    }

    fn undo(&mut self, before: Before) {
        let gone: Vec<_> = self
            .editor
            .edits
            .keys()
            .filter(|p| !before.edits.contains_key(*p))
            .cloned()
            .collect();
        for path in gone {
            self.editor.discard(&path);
        }
        for (path, text) in &before.edits {
            if self.editor.edits.get(path) != Some(text) {
                let _ = if path.exists() {
                    self.editor
                        .open(path)
                        .and_then(|()| self.editor.set(path, text.clone()))
                } else {
                    self.editor.create(path, text.clone())
                };
            }
        }
        self.added = before.added;
        self.files = before.files;
        self.editor.tabs = before.tabs;
        self.editor.active = before.active;
    }

    fn refresh_files(&mut self) -> Result<(), String> {
        let project = self.project.as_ref().ok_or("Load a skin first")?;
        self.files = files::with_added(files::inventory(project)?, project, &self.added);
        Ok(())
    }

    /// Where an imported file's copy is kept: the project checkpoint's `added/`.
    fn store_dir(&self) -> Result<PathBuf, String> {
        let home = self.home.as_ref().ok_or("No project home")?;
        let project = self.project.as_ref().ok_or("Load a skin first")?;
        Ok(workspace::dir(home, &project.dir).join("added"))
    }

    fn change(&mut self, change: &Change) -> Result<(), String> {
        match change {
            Change::Write {
                path,
                text,
                expect_hash,
            } => {
                let p = self.text_file(path)?;
                if let Some(expected) = expect_hash {
                    let current = match self.editor.edits.get(&p) {
                        Some(t) => content_hash(t),
                        None => content_hash(&files::read_text(&p)?),
                    };
                    if &current != expected {
                        return Err(format!(
                            "{path} changed since it was read (now {current}, expected {expected}); read it again"
                        ));
                    }
                }
                #[cfg(feature = "skincraft")]
                if self
                    .vector
                    .as_ref()
                    .is_some_and(|v| v.retained.contains_key(&p))
                {
                    return Err(format!(
                        "{path} is managed by the vector editor; release_svg_source first"
                    ));
                }
                if self.drawings.contains_key(&p) {
                    return Err(format!("{path} has a raster override; release it first"));
                }
                if self.editor.edits.contains_key(&p) || p.exists() {
                    self.editor.open(&p)?;
                }
                self.editor.set(&p, text.clone())
            }
            Change::Create { path, text } => {
                let p = self.own_path(path)?;
                if !files::is_text(&p.to_string_lossy()) {
                    return Err(format!(
                        "{path}: create makes text files (Lua, metrics, SVG…); import other files"
                    ));
                }
                if p.exists() || self.added.contains_key(&p) {
                    return Err(format!(
                        "{path} already exists in the skin; write it instead"
                    ));
                }
                self.editor.create(&p, text.clone())?;
                self.added.insert(p, project::Added::Text);
                self.refresh_files()
            }
            Change::Import {
                source,
                path,
                replace,
            } => {
                let p = self.own_path(path)?;
                if p.exists() {
                    return Err(format!(
                        "{path} is a file of the source skin; edit it instead of importing over it"
                    ));
                }
                if self.added.contains_key(&p) && !replace {
                    return Err(format!(
                        "{path} was added before; pass replace to change it"
                    ));
                }
                let from = Path::new(source);
                let size = std::fs::metadata(from)
                    .map_err(|e| format!("{source}: {e}"))?
                    .len();
                if size > MAX_IMPORT {
                    return Err(format!("{source} is larger than {} MiB", MAX_IMPORT >> 20));
                }
                let bytes = std::fs::read(from).map_err(|e| format!("{source}: {e}"))?;
                if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("svg")) {
                    let text =
                        std::str::from_utf8(&bytes).map_err(|_| format!("{source} is not text"))?;
                    ddi_noteskin::svg::check(text).map_err(|e| format!("{source}: {e}"))?;
                }
                let store = self.store_dir()?;
                std::fs::create_dir_all(&store).map_err(|e| e.to_string())?;
                let name = p.file_name().unwrap_or_default().to_string_lossy();
                let copy = store.join(format!("{}-{name}", ddi_noteskin::report::hash(&bytes)));
                workspace::atomic(&copy, &bytes)?;
                if self.editor.edits.contains_key(&p) {
                    self.editor.discard(&p);
                }
                self.added.insert(p, project::Added::File(copy));
                self.refresh_files()
            }
            Change::Remove { path } => {
                let p = self.own_path(path)?;
                match self.added.get(&p) {
                    Some(project::Added::Excluded) => Err(format!("{path} is left out already")),
                    Some(_) => {
                        self.added.remove(&p);
                        self.editor.discard(&p);
                        self.refresh_files()
                    }
                    // A file of the skin's own folder: left out, the folder untouched.
                    None if p.is_file() => {
                        self.editor.discard(&p);
                        self.added.insert(p, project::Added::Excluded);
                        self.refresh_files()
                    }
                    None => Err(format!("{path} is not a file of the skin")),
                }
            }
            Change::Restore { path } => {
                let p = self.own_path(path)?;
                if self.added.get(&p) != Some(&project::Added::Excluded) {
                    return Err(format!("{path} is not a file the project left out"));
                }
                self.added.remove(&p);
                self.refresh_files()
            }
            Change::Discard { path } => {
                let p = self.text_file(path)?;
                if self.added.contains_key(&p) {
                    return Err(format!(
                        "{path} was added by the project; remove it instead"
                    ));
                }
                if !self.editor.edits.contains_key(&p) {
                    return Err(format!("{path} has no draft"));
                }
                self.editor.discard(&p);
                Ok(())
            }
        }
    }

    /// `path` as a text file of the project (a source file's or an added one).
    pub(super) fn text_file(&self, path: &str) -> Result<PathBuf, String> {
        let given = Path::new(path);
        let p = if given.is_absolute() {
            given.canonicalize().unwrap_or_else(|_| given.to_path_buf())
        } else {
            self.own_path(path)?
        };
        if self.files.iter().any(|f| f.path == p && f.text) {
            Ok(p)
        } else {
            Err(format!("{path} is not a text file of this project"))
        }
    }

    /// The project's own files the skin's folder now holds with the same bytes (copied there,
    /// as when files are mirrored into the source): the folder's are used and the project's
    /// dropped, rather than refusing the skin for two files at one path. Their paths.
    pub(super) fn adopt_mirrored(&mut self) -> Vec<PathBuf> {
        let same: Vec<PathBuf> = self
            .added
            .iter()
            .filter(|(path, file)| {
                let Ok(folder) = std::fs::read(path) else {
                    return false;
                };
                let ours = match (self.editor.edits.get(*path), file) {
                    (Some(text), _) => Some(text.as_bytes().to_vec()),
                    (None, project::Added::File(copy)) => std::fs::read(copy).ok(),
                    (None, _) => None,
                };
                ours.is_some_and(|ours| ours == folder)
            })
            .map(|(path, _)| path.clone())
            .collect();
        for path in &same {
            self.added.remove(path);
            self.editor.discard(path);
        }
        if !same.is_empty() {
            self.checkpoint_dirty = true;
        }
        same
    }

    /// Applies `changes` as one revision: each checked, then the preview reloaded once through
    /// the game's loader. When a change or the reload fails, every change is undone and the last
    /// working preview kept; the error names the change.
    pub(super) fn apply(&mut self, changes: &[Change]) -> Result<(), String> {
        if changes.is_empty() {
            return Err("No changes".into());
        }
        if changes.len() > 256 {
            return Err("At most 256 changes at once".into());
        }
        let before = self.before();
        for (n, change) in changes.iter().enumerate() {
            if let Err(e) = self.change(change) {
                self.undo(before);
                return Err(format!("change {n} ({}): {e}", describe(change)));
            }
        }
        let checked = self.editor.edits.iter().try_for_each(|(path, text)| {
            if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("svg"))
            {
                ddi_noteskin::svg::check(text)
                    .map(|_| ())
                    .map_err(|e| format!("{}: {e}", path.display()))?;
            }
            Ok::<_, String>(())
        });
        if let Err(e) = checked.and_then(|()| self.load()) {
            self.undo(before);
            return Err(format!("the skin did not load with these changes: {e}"));
        }
        self.text_reload = None;
        self.checkpoint_dirty = true;
        self.thumbnails.clear();
        Ok(())
    }
}

fn describe(change: &Change) -> String {
    match change {
        Change::Write { path, .. } => format!("write {path}"),
        Change::Create { path, .. } => format!("create {path}"),
        Change::Import { path, .. } => format!("import {path}"),
        Change::Remove { path } => format!("remove {path}"),
        Change::Restore { path } => format!("restore {path}"),
        Change::Discard { path } => format!("discard {path}"),
    }
}
