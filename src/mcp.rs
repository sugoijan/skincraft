//! SkinCraft's stdio MCP facade. All tools address the same live workspace, including the
//! native editor command catalogs, in-progress strokes, source text and whole-window captures.
use photocraft_automation::BridgeClient;
use rmcp::handler::server::{router::tool::ToolRouter, wrapper::Parameters};
use rmcp::model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig};
use rmcp::{ServerHandler, ServiceExt, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;

#[derive(Clone)]
pub struct SkinCraftMcp {
    home: PathBuf,
    router: ToolRouter<Self>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PathParams {
    /// Absolute file or folder path. Project files are readable; other paths must be beneath SKINCRAFT_READ_ROOT or SKINCRAFT_WRITE_ROOT as appropriate.
    pub path: String,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WriteParams {
    /// A project text file: absolute, or relative to the skin's folder.
    pub path: String,
    pub text: String,
    /// The `hash` read_text returned: refused if the draft changed since (no lost edits).
    pub expect_hash: Option<String>,
    /// Return the whole text (default: only its hash and size).
    pub echo: Option<bool>,
    /// Any unique string per intended edit; sending the same id again returns the first reply
    /// instead of applying twice (safe retries after a timeout). Only requests that ran are
    /// replayed: a refused one may be corrected and sent again with its id.
    pub request_id: Option<String>,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReadParams {
    pub path: String,
    /// First line returned (1-based); with end_line, a slice of a large file.
    pub start_line: Option<u64>,
    pub end_line: Option<u64>,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InspectParams {
    /// summary (default), files, parameters, preview, images (each image's cost, hash and budget share), editors (both native editors' full state) or all.
    pub scope: Option<String>,
}
/// One change of an apply: an object with its `op`. Paths are in the skin's folder (relative
/// or absolute); import sources must be readable.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum ApplyChange {
    /// A project text file's new draft (a file of the skin or one the project added).
    Write {
        path: String,
        text: String,
        /// The hash read_text returned: refused if the draft changed since.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expect_hash: Option<String>,
    },
    /// A new text file (Lua, metrics, SVG…) in the skin's folder.
    Create { path: String, text: String },
    /// A file copied in from `source` as the project's own.
    Import {
        source: String,
        path: String,
        /// Replace a file the project imported before at `path`.
        #[serde(default)]
        replace: bool,
    },
    /// A file the project added, removed; a file of the skin's own folder, left out of the
    /// preview and exports (the folder untouched).
    Remove { path: String },
    /// A file of the skin's folder that was left out, back in.
    Restore { path: String },
    /// A source file's text draft dropped: back to the file.
    Discard { path: String },
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ApplyParams {
    /// Changes applied as one revision with one preview reload; if any fails or the skin no longer loads, none applies.
    pub changes: Vec<ApplyChange>,
    pub request_id: Option<String>,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FileParams {
    /// A path in the skin's folder, relative to it or absolute.
    pub path: String,
    /// create_file: the new file's text.
    pub text: Option<String>,
    /// import_file: the file copied in (beneath a read root).
    pub source: Option<String>,
    /// import_file: replace a file the project imported before at this path.
    pub replace: Option<bool>,
    pub request_id: Option<String>,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CaptureParams {
    /// Seconds into the demo loop (default: the preview's current time).
    pub time: Option<f64>,
    /// Pixels, 64 to 2048 (defaults 480 × 640).
    pub width: Option<u64>,
    pub height: Option<u64>,
    /// Optional PNG copy beneath the write root.
    pub path: Option<String>,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExportParams {
    pub path: String,
    pub request_id: Option<String>,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EditorParams {
    /// active (default), vector, or raster. Discover commands with method engine.commands.
    pub editor: Option<String>,
    /// engine.commands, engine.execute, document.inspect, ui.inspect, ui.set, ui.pointer, ui.tool.select, ui.tool.list, ui.menu.list or ui.render. Use engine.execute with params {command, params}. Pointer coordinates are document coordinates by default.
    pub method: String,
    pub params: Option<Value>,
    pub request_id: Option<String>,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PreviewParams {
    pub running: Option<bool>,
    pub time: Option<f64>,
    /// Compare autoplay/manual hit effects while keeping the demo's timing examples.
    pub autoplay_flashes: Option<bool>,
    /// itg, danoni, sm5 or ddr-a.
    pub ruleset: Option<String>,
    /// Parameter patch keyed by the ids from inspect_workspace. Null resets one to its default.
    pub parameters: Option<std::collections::BTreeMap<String, Value>>,
    /// dance-single, dance-solo, dance-double, danoni-5, danoni-7, danoni-9A, danoni-9B, pump-single or pump-double.
    pub layout: Option<String>,
    pub reverse: Option<bool>,
    /// 0.5 to 4 (an x-mod).
    pub scroll_speed: Option<f64>,
    /// The background's brightness, 0 to 1.
    pub backdrop: Option<f64>,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScreenshotParams {
    /// Optional PNG copy beneath the write root. An inline image is always returned.
    pub path: Option<String>,
}

impl SkinCraftMcp {
    pub fn new(home: PathBuf) -> Self {
        Self {
            home,
            router: Self::router(),
        }
    }
    async fn call(&self, method: &str, params: Value) -> CallToolResult {
        let result = async {
            let bytes = tokio::fs::read(self.home.join("control.json"))
                .await
                .map_err(|_| {
                    "SkinCraft is not running for this project home. Start ./run.sh.".to_string()
                })?;
            let descriptor: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            if descriptor["app"] != "SkinCraft" {
                return Err("Unexpected control server".into());
            }
            let token_path = descriptor["tokenFile"]
                .as_str()
                .ok_or("Missing token file")?;
            let token = photocraft_automation::security::client_token(
                None,
                Some(std::path::Path::new(token_path)),
            )
            .map_err(|e| e.to_string())?;
            let bridge = BridgeClient::new(
                descriptor["address"]
                    .as_str()
                    .ok_or("Missing server address")?,
                token,
            )
            .map_err(|e| e.to_string())?;
            bridge.call(method, params).await.map_err(|e| e.to_string())
        }
        .await;
        match result {
            Ok(mut value) => {
                let image = value
                    .as_object_mut()
                    .and_then(|v| v.remove("pngBase64"))
                    .and_then(|v| v.as_str().map(str::to_owned));
                let mut content = Vec::new();
                if let Some(png) = image {
                    content.push(ContentBlock::image(png, "image/png"));
                }
                content.push(ContentBlock::text(value.to_string()));
                CallToolResult::success(content)
            }
            Err(error) => CallToolResult::error(vec![ContentBlock::text(error)]),
        }
    }
    pub async fn serve_stdio(self) -> Result<(), String> {
        let service = self
            .serve(rmcp::transport::stdio())
            .await
            .map_err(|e| e.to_string())?;
        service.waiting().await.map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[tool_router(router = router)]
impl SkinCraftMcp {
    #[tool(
        description = "Health and identity in one small answer: app version, pid, project home and skin, active editor, preview revision, draft counts, whether anything is pending, the last failure, the skin's nonfatal warnings (scripts' missing files, failed commands), files adopted from the skin's folder, how the last export ended, and the images' budget. Check it first, and after a timeout before retrying.",
        annotations(read_only_hint = true, idempotent_hint = true)
    )]
    async fn status(&self) -> CallToolResult {
        self.call("status", json!({})).await
    }
    #[tool(
        description = "Inspect one part of the workspace: summary (default, small), files, parameters, preview, images (each image's decoded size, hash, budget share and why one was refused), editors (both native editors' full state, large) or all.",
        annotations(read_only_hint = true, idempotent_hint = true)
    )]
    async fn inspect_workspace(&self, Parameters(p): Parameters<InspectParams>) -> CallToolResult {
        self.call(
            "inspect",
            json!({"scope":p.scope.unwrap_or("summary".into())}),
        )
        .await
    }
    #[tool(
        description = "List every image, script, metrics file and inherited resource in the current noteskin, with whether the project added it and whether it has a draft.",
        annotations(read_only_hint = true, idempotent_hint = true)
    )]
    async fn list_files(&self) -> CallToolResult {
        self.call("list_files", json!({})).await
    }
    #[tool(
        description = "Open a project resource in its editor: SVG in VectorCraft, bitmap in PhotoCraft, script in text. Existing tabs and edits are reused. External artwork opens as an independent reference."
    )]
    async fn open_file(&self, Parameters(p): Parameters<PathParams>) -> CallToolResult {
        self.call("open_file", json!({"path":p.path})).await
    }
    #[tool(
        description = "Save the current project and load a noteskin folder beneath the configured read root."
    )]
    async fn load_skin(&self, Parameters(p): Parameters<PathParams>) -> CallToolResult {
        self.call("load_skin", json!({"path":p.path})).await
    }
    #[tool(
        description = "Read the current draft of a project text file (including files the project added), with its hash for write_text's expect_hash; start_line/end_line read a slice.",
        annotations(read_only_hint = true, idempotent_hint = true)
    )]
    async fn read_text(&self, Parameters(p): Parameters<ReadParams>) -> CallToolResult {
        self.call(
            "read_text",
            json!({"path":p.path,"start_line":p.start_line,"end_line":p.end_line}),
        )
        .await
    }
    #[tool(
        description = "Edit a project script, metrics or SVG text draft and reload the game preview at once. Replies with the new hash, size and preview revision (not the text, unless echo). If the skin no longer loads, the edit is refused and the last valid preview kept."
    )]
    async fn write_text(&self, Parameters(p): Parameters<WriteParams>) -> CallToolResult {
        self.call("write_text", json!({"path":p.path,"text":p.text,"expect_hash":p.expect_hash,"echo":p.echo,"requestId":p.request_id}))
            .await
    }
    #[tool(
        description = "Apply several file changes as one revision with one preview reload: writes, new text files, imported files, removals and discarded drafts. All or nothing: if one fails, or the skin does not load with them, nothing changes and the last working preview stays. The source folder is never written: added files live with the project and reach the preview and exports."
    )]
    async fn apply(&self, Parameters(p): Parameters<ApplyParams>) -> CallToolResult {
        let changes = serde_json::to_value(&p.changes).unwrap_or(Value::Null);
        self.call("apply", json!({"changes":changes,"requestId":p.request_id}))
            .await
    }
    #[tool(
        description = "Create a new text file (Lua, metrics, SVG…) in the skin's folder, or one overriding an inherited file of that name; refused if the skin already has it. Reloads the preview."
    )]
    async fn create_file(&self, Parameters(p): Parameters<FileParams>) -> CallToolResult {
        self.call(
            "create_file",
            json!({"path":p.path,"text":p.text,"requestId":p.request_id}),
        )
        .await
    }
    #[tool(
        description = "Copy a file (a generated SVG sheet, a PNG, a model) into the skin's folder as the project's own; never over a source file. replace changes one imported before. Reloads the preview."
    )]
    async fn import_file(&self, Parameters(p): Parameters<FileParams>) -> CallToolResult {
        self.call(
            "import_file",
            json!({"path":p.path,"source":p.source,"replace":p.replace,"requestId":p.request_id}),
        )
        .await
    }
    #[tool(
        description = "Remove a file the project added (an inherited file it overrode shows again), or leave a file of the skin's own folder out of the preview and exports (the folder itself is never changed; restore_file brings it back). Reloads the preview."
    )]
    async fn remove_file(&self, Parameters(p): Parameters<FileParams>) -> CallToolResult {
        self.call(
            "remove_file",
            json!({"path":p.path,"requestId":p.request_id}),
        )
        .await
    }
    #[tool(
        description = "Bring back a file of the skin's own folder that remove_file left out. Reloads the preview."
    )]
    async fn restore_file(&self, Parameters(p): Parameters<FileParams>) -> CallToolResult {
        self.call(
            "restore_file",
            json!({"path":p.path,"requestId":p.request_id}),
        )
        .await
    }
    #[tool(description = "Drop a source file's text draft, back to the file. Reloads the preview.")]
    async fn discard_draft(&self, Parameters(p): Parameters<FileParams>) -> CallToolResult {
        self.call(
            "discard_draft",
            json!({"path":p.path,"requestId":p.request_id}),
        )
        .await
    }
    #[tool(
        description = "Switch a project SVG from native vector editing to editable source text, preserving its last accepted SVG. Releases its session vector document/history."
    )]
    async fn release_svg_source(&self, Parameters(p): Parameters<PathParams>) -> CallToolResult {
        self.call("release_svg_source", json!({"path":p.path}))
            .await
    }
    #[tool(
        description = "Discover and run native editor commands, inspect objects/layers, select tools, or draw through ui.pointer. Use engine.commands first for exact command ids and parameters. File operations use SkinCraft tools."
    )]
    async fn editor_request(&self, Parameters(p): Parameters<EditorParams>) -> CallToolResult {
        self.call("editor_request",json!({"editor":p.editor.unwrap_or("active".into()),"method":p.method,"params":p.params.unwrap_or(json!({})),"requestId":p.request_id})).await
    }
    #[tool(
        description = "Save the whole SkinCraft project with editable vector/raster sources, script drafts and added files."
    )]
    async fn save_project(&self) -> CallToolResult {
        self.call("save_project", json!({})).await
    }
    #[tool(
        description = "Save an independent copy of the active artwork. Vector copies use .svg or .vectorcraft; raster copies use .pcraft, PSD or image formats. Never overwrites original noteskin resources."
    )]
    async fn save_file(&self, Parameters(p): Parameters<PathParams>) -> CallToolResult {
        self.call("save_file", json!({"path":p.path})).await
    }
    #[tool(
        description = "Export a game-loadable noteskin into a new destination below the write root, with drafts and added files, without the files the project left out. A pending reload runs first; if a source still cannot load, the error lists each blocker with its file and stage. If the request times out, status's lastExport says how the export ended: check it before exporting into the same destination again."
    )]
    async fn export_skin(&self, Parameters(p): Parameters<ExportParams>) -> CallToolResult {
        self.call(
            "export_skin",
            json!({"path":p.path,"requestId":p.request_id}),
        )
        .await
    }
    #[tool(
        description = "Set declared skin parameters, the layout, reverse, scroll speed, backdrop and ruleset, pause/resume or seek the shared game preview. Parameters are preview choices; exported defaults stay unchanged."
    )]
    async fn set_preview(&self, Parameters(p): Parameters<PreviewParams>) -> CallToolResult {
        self.call(
            "preview",
            json!({"running":p.running,"time":p.time,"autoplay_flashes":p.autoplay_flashes,"ruleset":p.ruleset,"parameters":p.parameters,"layout":p.layout,"reverse":p.reverse,"scroll_speed":p.scroll_speed,"backdrop":p.backdrop}),
        )
        .await
    }
    #[tool(
        description = "Render the game preview offscreen at a time and size and return it as a PNG, with the revision and images hash it shows. Needs no visible window; screenshot captures the whole window instead.",
        annotations(read_only_hint = true)
    )]
    async fn capture_preview(&self, Parameters(p): Parameters<CaptureParams>) -> CallToolResult {
        self.call(
            "capture_preview",
            json!({"time":p.time,"width":p.width,"height":p.height,"path":p.path}),
        )
        .await
    }
    #[tool(
        description = "Capture the whole SkinCraft window, including active editor, files and actual game preview. Returns an inline PNG. The window must be visible; capture_preview renders the preview alone without it."
    )]
    async fn screenshot(&self, Parameters(p): Parameters<ScreenshotParams>) -> CallToolResult {
        self.call("screenshot", json!({"path":p.path})).await
    }
}

#[tool_handler(router = self.router)]
impl ServerHandler for SkinCraftMcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("skincraft",env!("CARGO_PKG_VERSION")))
            .with_instructions("SkinCraft noteskin workspace. Start with status, then inspect_workspace (scope summary, files, images…) and list_files. Open project images to edit SVGs in VectorCraft or bitmaps in PhotoCraft; editor_request engine.commands discovers exact native command ids. Scripts and metrics use read_text/write_text (pass read_text's hash as expect_hash). New and generated files: create_file, import_file, or several changes as one revision with apply. capture_preview renders the game preview offscreen. Save project preserves editable sources; export_skin writes a new runtime tree. Default file reads are limited to bundled noteskins and the current project's files; writes are limited to the project home. SKINCRAFT_READ_ROOT/WRITE_ROOT configure additional working roots when starting the app. Give mutating calls a request_id: retrying with the same id after a timeout returns the first reply instead of applying twice; otherwise check status before retrying.")
    }
}

pub fn project_home() -> Result<PathBuf, String> {
    if let Some(home) = std::env::var_os("DDI_STUDIO_HOME") {
        return Ok(PathBuf::from(home));
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME")
            .map(|h| PathBuf::from(h).join("Library/Application Support/SkinCraft"))
            .ok_or("No project home".into())
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA")
            .map(|h| PathBuf::from(h).join("SkinCraft"))
            .ok_or("No project home".into())
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .map(|h| h.join("skincraft"))
            .ok_or("No project home".into())
    }
}
