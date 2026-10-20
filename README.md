# DocCraft — the real crafts, one suite

[![Build & Release](https://github.com/20kiwy/DocCraft/actions/workflows/build.yml/badge.svg)](https://github.com/20kiwy/DocCraft/actions/workflows/build.yml)
[![Release](https://img.shields.io/github/v/release/20kiwy/DocCraft?include_prereleases)](https://github.com/20kiwy/DocCraft/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](#licenses)

**One binary hosting the FIVE real storytold crafts — WordCraft, GridCraft,
DeckCraft, PdfCraft and FigCraft — plus four new native crafts: Draw (drawio),
Markdown, Typeset and Notes.** The upstream apps are linked **unchanged**
from git submodules under [`clone/`](clone) and render their own complete,
real UIs (their chrome, ribbons, toolbars, panels and status bars —
pixel-for-pixel theirs). The shell (launcher, hub strip, About, window header)
is themed after storytold's own **craft-launcher**.

| ![Launcher](screenshots/00-launcher.png) |
|:---:|
| **DocCraft launcher** — all nine crafts, craft-launcher theme, per-craft status |

## The nine crafts

| # | Craft | Opens | Source | Version | Honest readiness |
|---|-------|-------|--------|---------|------------------|
| 1 | **Writer** (WordCraft) | `.docx .doc .odt .rtf` | [storytold/wordcraft](https://github.com/storytold/wordcraft) | 0.8.0 | ~60% — ribbon catalog **97%** |
| 2 | **Sheets** (GridCraft) | `.xlsx .xls .csv …` | [storytold/gridcraft](https://github.com/storytold/gridcraft) | 0.8.0 | breadth ~75% — functions **98.1%** |
| 3 | **Slides** (DeckCraft) | `.pptx …` | [storytold/deckcraft](https://github.com/storytold/deckcraft) | 0.8.0 | breadth 84% — ready ~55% |
| 4 | **Reader** (PdfCraft) | `.pdf` | [storytold/pdfcraft](https://github.com/storytold/pdfcraft) | 0.9.0 | ready ~50% |
| 5 | **Design** (FigCraft) | `.fig .figcraft .figma .svg` | [storytold/figcraft](https://github.com/storytold/figcraft) | 0.7.0 | menus **100%** — ready ~53% |
| 6 | **Draw** (drawio-style) | `.drawio .xml` | `crates/drawio` (this repo) | 3.4.0 | native — full GraphEditor UI, mxGraph XML round-trip, drawio-exact hover arrows / green connection dots / edge start-end editing, scratchpad + drag-from-palette, full export |
| 7 | **Markdown** | `.md .markdown` | `crates/md` (this repo) | 3.4.0 | native — split live preview on a page card, shared UI3 design tokens |
| 8 | **Typeset** | `.typ .tex` | `crates/typeset` (this repo) | 3.4.0 | native — justified pagination → PDF, paginated page cards, shared UI3 design tokens |
| 9 | **Notes** | `.note .notes` | `crates/notes` (this repo) | 3.4.0 | native — multi-page notebook, styled live preview, settings, .note JSON save/load |

Readiness numbers come from each upstream repo's own `ROADMAP.md` — see
[`CRAFT-STATUS.md`](CRAFT-STATUS.md) for the full honest table.

### One design language

Since 3.4 every native craft paints with the same design tokens as the
upstream crafts: the FigCraft UI3 light/dark system lives in
[`crates/theme`](crates/theme) (shared crate `doccraft-theme`) — accent
`#0D99FF`, radius-5 widgets, radius-13 windows, soft page shadows — and each
craft has a ☀/☾ theme toggle. Draw is a full GraphEditor (menus, toolbar,
searchable shape libraries + Scratchpad with drag-to-canvas, Format panel with
Diagram/Style/Text/Arrange tabs, page tabs, status bar), and Markdown /
Typeset / Notes show their output as real page cards on a canvas.

## Screenshots

| Writer (real WordCraft) | Sheets (real GridCraft) |
|:---:|:---:|
| ![Writer](screenshots/01-wordcraft.png) | ![Sheets](screenshots/02-gridcraft.png) |
| **Slides** (real DeckCraft) | **Reader** (real PdfCraft) |
| ![Slides](screenshots/03-deckcraft.png) | ![Reader](screenshots/04-pdfcraft.png) |
| **Design** (real FigCraft) | **Draw** (native drawio editor) |
| ![Design](screenshots/05-figcraft.png) | ![Draw](screenshots/06-draw.png) |
| **Markdown** | **Typeset** |
| ![Markdown](screenshots/08-markdown.png) | ![Typeset](screenshots/09-typeset.png) |
| **Notes** | **About DocCraft** |
| ![Notes](screenshots/10-notes.png) | ![About](screenshots/11-about.png) |
| **drawio green connection dots** | **Edge mid-drag** (× fixed points + green snap ring + target highlight) |
| ![Hover](screenshots/12-draw-hover.png) | ![Drag](screenshots/13-draw-drag.png) |
| **Edge linked** (start/end editable in Style panel) | **drawio hover arrows** (exact #29b6f2 SVG + placement) |
| ![Linked](screenshots/14-draw-linked.png) | ![Arrows](screenshots/15-draw-arrows.png) |
| **Connect mid-drag** (dashed-green preview + × points + [100%, 50%] hint) | **Connected at fixed point** |
| ![MidDrag](screenshots/16-draw-connect-middrag.png) | ![Connected](screenshots/17-draw-connected.png) |
| **Endpoint re-link mid-drag** (drawio edge-handler hint) | **Re-linked** (start/end changed like drawio) |
| ![RelinkMid](screenshots/18-draw-relink-middrag.png) | ![Relinked](screenshots/19-draw-relinked.png) |

## Why this is "same same"

- The UI you get is **the upstream UI code itself** (`crates/ui-egui` of each
  repo — WordCraft's alone is 54k lines across ~60 modules), not a re-creation.
- All crafts share storytold's own UI foundation
  [`craft-ui`](https://github.com/storytold/craft-libs) — patched to one local
  checkout so the suite links a single copy.
- Each craft keeps its own engine, model and file readers; opening a `.docx`
  runs WordCraft's real `file.open` command, a `.pptx` runs DeckCraft's, etc.
- The drawio craft implements the **jgraph/drawio v32.5.5** connection spec
  natively in Rust (hover arrows, × fixed points, green snap ring, edge
  start/end re-binding, 32-entry arrow marker list, SVG/PNG/PDF export) —
  the extracted spec lives in [`DRAWIO-SPEC.md`](DRAWIO-SPEC.md).

## Download (per-version release binaries)

Every `v*` tag triggers CI that builds **all four platform binaries** and
attaches them to a GitHub Release automatically:

| Artifact | Platform |
|---|---|
| `doccraft-<version>-linux-x64.zip` | Linux x86_64 (glibc 2.35+) |
| `doccraft-<version>-windows-x64.zip` | Windows x86_64 (+ `install.bat` / `install.ps1` — registers 27 file associations, no admin needed) |
| `doccraft-<version>-macos-x64.zip` | macOS Intel |
| `doccraft-<version>-macos-arm64.zip` | macOS Apple Silicon |

→ **[Latest release](https://github.com/20kiwy/DocCraft/releases/latest)**

macOS note: the binaries are unsigned — right-click → Open the first time, or
`xattr -d com.apple.quarantine doccraft`.

## Build & run

```bash
git clone --recurse-submodules https://github.com/20kiwy/DocCraft
cargo build --profile fast      # 2-core-friendly profile (opt 1, no LTO)
./target/fast/doccraft          # launcher opens; pick a craft
./target/fast/doccraft report.docx plan.drawio notes.md   # open straight into a craft
```

Windows: `cargo build --profile fast` on MSVC needs NASM (`choco install nasm`)
for `aws-lc-sys`. Or just use the CI-built release zip.

Linux build/run deps: `libxkbcommon-dev libwayland-dev` (dlopened at runtime;
software GL works fine — `WGPU_BACKEND=gl`).

## Updating the upstream crafts

```bash
scripts/update-crafts.sh              # all six repos → latest main
scripts/update-crafts.sh wordcraft    # just one
cargo build --profile fast            # rebuild the suite against the new pins
```

Each `clone/<repo>` is a normal git submodule: `git submodule status`
prints the pinned SHA, `git -C clone/wordcraft checkout vX.Y.Z` pins a
release, and `git add clone/wordcraft` records it. `scripts/apply-overrides.sh`
re-applies the DocCraft system edits (single About, Website link) after any
update — idempotent.

## Command line

```
doccraft [--sample] [files…] [--register-associations (Windows)]
```

Files route by extension: `.docx .doc .odt` → Writer · `.xlsx .csv …` →
Sheets · `.pptx` → Slides · `.pdf` → Reader · `.fig .figcraft .svg` → Design ·
`.drawio .xml` → Draw · `.md` → Markdown ·
`.typ .tex` → Typeset · `.note .notes` → Notes.

### Windows file associations

`windows/install.bat` (or `install.ps1`) copies DocCraft to
`%LOCALAPPDATA%\DocCraft`, registers **26 extensions** (docx doc odt rtf
xlsx xlsm xls ods csv tsv pptx potx ppsx pdf fig figcraft svg drawio xml
md markdown typ tex ts note notes) under `HKCU\Software\Classes` with icons
and a Start-Menu shortcut — no admin required. `uninstall.bat` removes
everything. From inside the app: **About → Register file associations**.

## Layout

```
DocCraft/
├── app/                    the DocCraft shell: launcher, hub strip, file routing,
│                           craft-launcher theme, register-associations
├── crates/
│   ├── drawio/             native drawio craft (mxGraph model + XML + connections + export)
│   ├── md/                 native Markdown craft (parser + live split preview)
│   ├── typeset/            native typesetting craft (layout engine + PDF export)
│   └── notes/              native notebook craft
├── clone/                  git submodules — the real repos, updatable
│   ├── craft-libs/         shared craft-ui widgets   (v0.1.0, 82a24b0)
│   ├── wordcraft/          Word                      (v0.8.0, 4a251d4)
│   ├── gridcraft/          Excel                     (v0.8.0-4, c30fa3f)
│   ├── deckcraft/          PPT                       (v0.8.0, 22db926)
│   ├── pdfcraft/           Acrobat                   (v0.9.0-38, a12fe8f)
│   ├── figcraft/           Figma-style Design        (v0.7.0-104, d8ff144)
│   └── drawio/             jgraph/drawio JS          (v32.5.5, 17b367b) — reference spec
├── screenshots/            all craft screenshots (below)
├── windows/                install.bat / install.ps1 / uninstall.bat
├── scripts/update-crafts.sh   pull every upstream repo to latest + re-pin
├── CRAFT-STATUS.md         versions + honest readiness per repo (from their ROADMAPs)
├── DRAWIO-SPEC.md          drawio JS connection/arrow/export spec → native implementation
└── PROGRESS.md             what was built and how it links
```

## Licenses

Upstream crafts: MIT OR Apache-2.0 (see each repo). DocCraft glue: MIT.
