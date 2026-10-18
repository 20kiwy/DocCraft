# DocCraft 3.1 — Progress & Shift-of-Things

**"Check figcraft well for drawio well, make the drawio well add more
function, remake the ui-panel for markdown and typeset with settings well."**

## What changed 3.0 → 3.1

| Area | 3.0 | 3.1 (this release) |
|---|---|---|
| Design view | — | **FigCraft embedded as the 5th real craft** (`upstream/figcraft` v0.7.0-104, 16 crates, ~236k lines): frames, vectors, text, components, styles, prototyping, Dev Mode; `.fig`/`.figcraft`/`.svg` routing |
| Draw (drawio) | palette of 8 mini-categories, basic editor | **full editor**: 14-category shape library (~180 shapes: General/Basic/Arrows/UML/ER/Flowchart/BPMN/Mockups/Tables/Titles/Misc), palette search, Style/Text/Arrange format tabs, layers panel, minimap outline, Edit-XML dialog, Find & Replace, group/ungroup, duplicate, alignment guides + snap, waypoints, edge routes (ortho/straight/curved) with line jumps, ER arrow ends, context menu, page setup, zoom controls, keyboard shortcuts |
| Markdown | toolbar + split preview | **wordcraft-anatomy chrome**: title strip + QAT, Home/Insert/View ribbon tabs with grouped commands, right pane (outline, document stats, export), settings dialog (editor/preview/accent/session), pipe tables, bottom preview mode, status bar (Ln/Col, words) |
| Typeset | source + A4 preview | **wordcraft-anatomy chrome**: ribbon tabs (Home/Insert/Page/View), settings-driven layout engine (paper presets, margins, 2 columns, page numbers, footer text, line spacing), right pane (outline with page jumps, recompile status, export), find bar, gutter, settings dialog |
| egui | 0.36 | 0.36 (unchanged) |

## Honest upstream status (from each repo's own ROADMAP)

See CRAFT-STATUS.md — FigCraft v0.7.0: menu breadth 100% (212/212), feature
breadth ~85% (core editor ~94%), ready ~53%. The crafts are not 100% complete;
embedding them gives DocCraft exactly their capability at the pinned commit.

---

# DocCraft 3.0 — Progress & Shift-of-Things

**"Copy all the crates from all the systems into one — with git so every repo
stays updatable."** This release does exactly that: the four real storytold
crafts are linked into one binary from git submodules, plus three new native
crafts (Draw, Markdown, Typeset) written in the same pattern.

## 1. What changed from DocCraft 2.5 → 3.0

| Area | 2.5 (previous) | 3.0 (this release) |
|---|---|---|
| UI source | our re-creation of the craft look | **the real upstream UI code** — each craft's own `crates/ui-egui` (WordCraft's is 54k lines) linked as path deps |
| Word view | our Writer mimic | **WordApp** from `upstream/wordcraft` (v0.8.0) — real ribbon, backstage, dialogs |
| Grid view | our Sheets mimic | **SheetApp** from `upstream/gridcraft` (v0.8.0) — real formula engine (513 functions) |
| Deck view | our Slides mimic | **SlideApp** from `upstream/deckcraft` (v0.8.0) — real PowerPoint engine |
| Reader view | our Reader mimic | **PdfCraftApp** from `upstream/pdfcraft` (v0.9.0) — real Acrobat-style shell |
| Shared widgets | n/a | **craft-ui** from `upstream/craft-libs` — the shared storytold UI foundation, one local checkout via `[patch]` |
| Draw | in-crate drawio parser + editor | **`crates/drawio`** native craft: mxGraph model, XML round-trip, diagrams.net-style editor |
| Markdown | in-crate notes view | **`crates/md`** native craft: CommonMark subset, split live preview |
| Typeset | in-crate typeset view | **`crates/typeset`** native craft: layout engine, paginated A4 preview, PDF export |
| Repo updates | manual copy | **git submodules** + `scripts/update-crafts.sh` |
| egui | 0.29 | **0.36** (what upstream uses) |
| Profiles | release-only | `fast` profile (opt 1, 256 codegen units, incremental) for 2-core builds |

## 2. How the linking works

```
app (doccraft binary)
 ├─ wordcraft-ui-egui  → upstream/wordcraft/crates/ui-egui  (+ engine, doc, docx, layout, render, fonts, proof, zotero, geom …)
 ├─ gridcraft-ui-egui  → upstream/gridcraft/crates/ui-egui  (+ engine, functions, formula, calc, numfmt, xlsx, chart …)
 ├─ deckcraft-ui-egui  → upstream/deckcraft/crates/ui-egui  (+ engine, pptx, model, render, text, media stack …)
 ├─ pdfcraft-ui-egui   → upstream/pdfcraft/crates/ui-egui   (+ engine, model, cos, filters, forms, ocr, sign …)
 ├─ craft-ui           → upstream/craft-libs/crates/ui       (single copy via [patch."…craft-libs"])
 ├─ doccraft-drawio / doccraft-md / doccraft-typeset       (this repo's native crafts)
```

- Package names in upstream are already namespaced (`wordcraft-*`,
  `gridcraft-*`, `deckcraft-*`, `pdfcraft-*`), so all four graphs coexist in
  one dependency graph with zero renames.
- `upstream/*` is `exclude`d from our workspace; cargo still resolves each
  crate's `workspace = true` inheritance from its own upstream root.
- Embedding surface per craft (all pub): `WordApp::new(session, services)` +
  `logic(ctx)` + `ui(ui)`; `SheetApp::new(session, services)` +
  `setup_context_for_language(ctx, dark, lang)`; `SlideApp::new(session,
  services)`; `PdfCraftApp::new()` + `open_bytes(...)` (rendered through its
  own `eframe::App::ui`). Services are constructed all-`None` (hosted mode —
  the upstream UIs already fall back to built-in file browsers/clipboards).
- Files route by extension through each craft's **own** command API:
  `file.open` (word/grid/deck) and `open_bytes` (pdf).

## 3. Status

| # | Phase | Status |
|---|---|---|
| 1 | Survey all 5 storytold repos, record versions + readiness (see CRAFT-STATUS.md) | ✅ |
| 2 | Workspace scaffold + 5 git submodules | ✅ |
| 3 | Shell app: launcher, hub strip, file routing, lazy craft mounting | ✅ |
| 4 | Embed WordCraft / GridCraft / DeckCraft / PdfCraft | ✅ wired — build in progress |
| 5 | Native crates: drawio (mxGraph XML round-trip editor) | ✅ |
| 6 | Native crates: markdown (CommonMark + live preview) | ✅ |
| 7 | Native crates: typeset (layout engine + PDF export) | ✅ |
| 8 | `cargo build --profile fast` (2-core) | 🔄 running |
| 9 | Headless Xvfb screenshots of every craft view | ⏳ |
| 10 | ZIP + manifests → download/, source mirror → public/ | ⏳ |

## 4. Build notes (2-core sandbox)

- `cargo build --profile fast` — opt-level 1, LTO off, 256 codegen units,
  incremental on. First build is big (upstream total ≈ 843k lines of Rust);
  it resumes across interruptions and caches per crate.
- Sandbox has no GTK3: rfd's portal backend (`xdg-portal`) is used via a
  vendor patch (see `vendor/rfd`) — pure Rust, no system GTK needed.
- Renderer: eframe with `wgpu` feature; headless runs use `WGPU_BACKEND=gl`
  (EGL) under Xvfb, same recipe as the earlier DocCraft builds
  (`LD_LIBRARY_PATH=/home/z/local-libs`).
