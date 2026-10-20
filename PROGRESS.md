# 3.3.0 — drawio connection parity + Vector craft removed

- **Vector (Illustrator-style) craft removed** per user request: the suite is
  now NINE crafts (5 real + Draw/Markdown/Typeset/Notes). `.vec` routing and
  the file association were dropped; `.svg` now routes to Design (FigCraft).
- **Draw craft matched to the real drawio JS** (jgraph/drawio v32.5.5, now a
  git submodule at `clone/drawio` so the spec source ships with the repo):
  - hover arrows = exact HoverIcons SVG paths/sizes/placement (Graph.js
    `HoverIcons.prototype.repaint`), 140 ms activation, 15% inactive opacity
  - preview edge = mxConnectionHandler `createShape` semantics: DASHED, green
    #00FF00 w3 over a valid target, red #FF0000 w1 over empty space, no markers
  - drop-target outline: green w5 when snapped to a fixed point
    (OUTLINE_HIGHLIGHT_COLOR), blue #00a8ff w5 @50% on plain valid hover
  - the drawio `[x%, y%]` hint (Graph.js `mxEdgeHandler.prototype.updateHint`
    + geHint CSS) shows while connecting/re-linking at a fixed point
  - connections now start on mousedown (drawio parity) — egui 0.36 postpones
    `drag_started` for click_and_drag widgets, which killed drags that left
    the hover zone before the decision; click-vs-drag is resolved at release
    (no movement = drawio's "click an arrow → new connected node")
  - edge-endpoint handles beat hover arrows when they coincide (drawio overlay
    order); endpoint re-binding tested end-to-end
  - upstream issues referenced: jgraph/drawio #1029 (tight spaces), #5627
    (1px-shifted points), #3469 (rotated constraints), #5683 (scale merge)
- **Automated test**: `crates/drawio/examples/connect-demo.rs` +
  `shots/connect-test.sh` (Xvfb + xdotool drag-and-drop; asserts the edge
  really LINKS with pinned exit/entry, then re-links the endpoint).
- Version 3.3.0.

# DocCraft 3.1 — Progress & Shift-of-Things

**"Check figcraft well for drawio well, make the drawio well add more
function, remake the ui-panel for markdown and typeset with settings well."**

## What changed 3.0 → 3.1

| Area | 3.0 | 3.1 (this release) |
|---|---|---|
| Design view | — | **FigCraft embedded as the 5th real craft** (`clone/figcraft` v0.7.0-104, 16 crates, ~236k lines): frames, vectors, text, components, styles, prototyping, Dev Mode; `.fig`/`.figcraft`/`.svg` routing |
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
| Word view | our Writer mimic | **WordApp** from `clone/wordcraft` (v0.8.0) — real ribbon, backstage, dialogs |
| Grid view | our Sheets mimic | **SheetApp** from `clone/gridcraft` (v0.8.0) — real formula engine (513 functions) |
| Deck view | our Slides mimic | **SlideApp** from `clone/deckcraft` (v0.8.0) — real PowerPoint engine |
| Reader view | our Reader mimic | **PdfCraftApp** from `clone/pdfcraft` (v0.9.0) — real Acrobat-style shell |
| Shared widgets | n/a | **craft-ui** from `clone/craft-libs` — the shared storytold UI foundation, one local checkout via `[patch]` |
| Draw | in-crate drawio parser + editor | **`crates/drawio`** native craft: mxGraph model, XML round-trip, diagrams.net-style editor |
| Markdown | in-crate notes view | **`crates/md`** native craft: CommonMark subset, split live preview |
| Typeset | in-crate typeset view | **`crates/typeset`** native craft: layout engine, paginated A4 preview, PDF export |
| Repo updates | manual copy | **git submodules** + `scripts/update-crafts.sh` |
| egui | 0.29 | **0.36** (what upstream uses) |
| Profiles | release-only | `fast` profile (opt 1, 256 codegen units, incremental) for 2-core builds |

## 2. How the linking works

```
app (doccraft binary)
 ├─ wordcraft-ui-egui  → clone/wordcraft/crates/ui-egui  (+ engine, doc, docx, layout, render, fonts, proof, zotero, geom …)
 ├─ gridcraft-ui-egui  → clone/gridcraft/crates/ui-egui  (+ engine, functions, formula, calc, numfmt, xlsx, chart …)
 ├─ deckcraft-ui-egui  → clone/deckcraft/crates/ui-egui  (+ engine, pptx, model, render, text, media stack …)
 ├─ pdfcraft-ui-egui   → clone/pdfcraft/crates/ui-egui   (+ engine, model, cos, filters, forms, ocr, sign …)
 ├─ craft-ui           → clone/craft-libs/crates/ui       (single copy via [patch."…craft-libs"])
 ├─ doccraft-drawio / doccraft-md / doccraft-typeset       (this repo's native crafts)
```

- Package names in upstream are already namespaced (`wordcraft-*`,
  `gridcraft-*`, `deckcraft-*`, `pdfcraft-*`), so all four graphs coexist in
  one dependency graph with zero renames.
- `clone/*` is `exclude`d from our workspace; cargo still resolves each
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


---

# DocCraft Suite 3.2 — Task 13 (Windows + drawio connections + launcher)

## What changed in 3.2
1. **TEN crafts in one binary** — the suite now hosts: Writer (WordCraft),
   Sheets (GridCraft), Slides (DeckCraft), Reader (PdfCraft), Design (FigCraft),
   Draw (drawio-native), Vector (Illustrator-native), Markdown, Typeset and Notes.
   Vector and Notes are new native crafts (crates/vector, crates/notes).
2. **drawio connection system re-derived from the real drawio/mxGraph JS**
   (see DRAWIO-SPEC.md): hover connection arrows (#29b6f2) appear after 140 ms,
   dragging one shows fixed-point × markers + blue valid-target highlight
   (#00a8ff w5 50%) + green snap ring (#00FF00 50%); dropping LINKS the edge to
   both shapes (source/target ids survive shape moves; exitX/exitY/entryX/entryY
   pinned when snapped to a point). Edge endpoints are draggable handles that
   re-link or dangle. Style panel edge section: Route (Orthogonal/Elbow H-V/
   Entity Relation/Curved/Straight), **Line start / Line end** full drawio marker
   list (classic/block/open/oval/diamond/ERone/ERmany/… with fill variants),
   Size s/e (marker sizes), Offset (perimeter spacing), Line Jumps
   (none/arc/gap/sharp/line + size), width/dash/opacity/colors.
3. **Full export** from Draw: .drawio XML (round-trip), SVG, PNG (software
   rasterizer + bitmap font) and PDF (vector writer), with zoom/border/
   transparent options.
4. **Craft-launcher chrome**: the shell now copies the storytold craft-launcher
   (ArtCraft Launcher v0.5.1) design system — dark #121316 tokens, square
   corners, 1px hairlines, uppercase Geist-mono-style labels, tracked display
   headings, custom 48 px header (logo mark + min/max/close window buttons,
   drag-to-move), launcher card grid on shared hairlines with the 0.3 s hover
   motion (app-color rule wipe, icon lift), footer with WEBSITE/GITHUB links,
   and ONE About window (About DocCraft) listing every craft's honest readiness.
5. **Discord → Website**: WordCraft's title-bar community pill now reads
   "Website" and opens github.com/storytold (scripts/apply-overrides.sh, idempotent,
   re-applied automatically by scripts/update-crafts.sh).
6. **Windows binary + file registration**: cargo-xwin cross-build
   (x86_64-pc-windows-msvc, aws-lc-sys built NO_ASM for the cross target —
   documented local overrides in the registry crate sources). Delivered:
   DocCraft.exe (207 MB PE32+ x64), install.bat / install.ps1 (copies to
   %LOCALAPPDATA%\DocCraft and registers Open-with for 27 file types via HKCU,
   no admin needed), uninstall.bat, doccraft.ico. In-app: About panel button +
   `DocCraft.exe --register-associations` do the same registration.
7. **Git**: submodules re-attached after a repo-history rebuild (the old .git
   had 2.3 GB of accidentally-committed build artifacts — now .gitignored).
   figcraft was restored as a proper gitlink at v0.7.0-104 (d8ff144).

## Windows build recipe (for rebuilds)
```
rustup target add x86_64-pc-windows-msvc
cargo install cargo-xwin            # + clang-19/lld-19/llvm-19 + ninja on PATH
# local overrides (already applied to the registry sources on this machine):
#   aws-lc-sys builder: allow AWS_LC_SYS_NO_ASM at any opt level
#   aws-lc hrss.c: exclude SSE2 union-op block under clang-cl (_MSC_VER)
export AWS_LC_SYS_NO_ASM=1
cargo xwin build --profile fast --target x86_64-pc-windows-msvc -p doccraft
# → target/x86_64-pc-windows-msvc/fast/deps/doccraft.exe
```
Crypto runs without assembly under this cross build (slower, still correct);
a native Windows build with NASM is the upstream-faithful alternative.

## Honest status (unchanged from 3.1 unless noted)
| craft | version | ready | notes |
|---|---|---|---|
| Writer/WordCraft | v0.8.0 | ~60% | real app embedded |
| Sheets/GridCraft | v0.8.0 | ~75% | functions 98.1% |
| Slides/DeckCraft | v0.8.0 | ~55% | real app embedded |
| Reader/PdfCraft | v0.9.0 | ~50% | real app embedded |
| Design/FigCraft | v0.7.0 | ~53% | 9th system fully embedded |
| Draw/drawio-native | 3.4.0 | — | full GraphEditor + drawio-exact connections (Vector removed in 3.3) |
| Markdown | 3.4.0 | — | wordcraft-anatomy chrome on UI3 tokens + page-card preview |
| Typeset | 3.4.0 | — | pagination + real PDF on UI3 tokens |
| Notes | 3.4.0 | — | full ribbon/preview/settings parity + .note JSON persistence |
| craft-launcher chrome | 0.5.1 (ref) | — | tokens/motion/header copied |

## 3.4.0 — one design language, full GraphEditor

- `crates/theme` (doccraft-theme): FigCraft UI3 light/dark Tokens ported 1:1
  from `clone/figcraft/crates/ui-egui/src/theme.rs` — accent #0D99FF,
  radius-5 widgets, radius-13 windows, soft page shadows, 11px UI text. Every
  native craft now paints exclusively from these tokens (dark default,
  ☀/☾ toggle, Light/Dark radio in settings).
- Draw = full GraphEditor: Arrange + Help menus, File New/Save As/Close,
  multi-open shape libraries, Scratchpad (auto-add on copy, right-click add,
  hover ✕ remove), drag shapes from the palette onto the canvas with a ghost
  preview, page-tab context ops (rename/duplicate/delete/insert), Diagram tab
  (paper/background/grid) when nothing is selected, toolbar zoom menu.
- Markdown/Typeset: UI3 restyle; preview/pagination render as page cards on a
  canvas (white/dark paper + card shadow); DocCraft Blue default accent.
- Notes rebuilt to full parity: glyph command strip, styled markdown preview
  on a page card, settings window with accent picker, .note JSON save/load
  (Ctrl+S) + text import.
- `.figma` files open via Design (FigCraft); Windows associations refreshed
  (27 types, stale .vec removed); About window file-type list updated.
