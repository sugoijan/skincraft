# SkinCraft

Native Rust/egui editor combining PhotoCraft, VectorCraft, text editing and
DDI's actual noteskin renderer. Read README.md and docs/architecture.md.

- Production dependencies use Git URLs with exact revisions and Cargo.lock.
- Editor/game code stays in its own repository; keep embedding hooks generic.
- scripts/local-patches.py generates ignored machine-local overrides.
- scripts/dev.py uses .local/dev and a separate lockfile. Never commit local paths.
- DDI stages assets beside the executable. Do not depend on sibling asset paths.
- Native tests isolate DDI_HOME and PHOTOCRAFT_CONFIG_DIR under target/scratch;
  use a watchdog and prefer the existing offscreen combined-editor/MCP tests.
- Build the binary before the MCP test: it launches the actual stdio bridge.
- Verify local overrides before pushing, then repeat with pinned Git dependencies.
- Do not overwrite original skin resources; maintain draft/last-valid-preview behavior.
- Keep user checkpoints, profiles, screenshots, caches and local patches ignored.
