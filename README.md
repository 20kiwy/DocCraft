# DocCraft 3.1 — the real crafts, one suite

**One binary hosting the FIVE real storytold crafts — WordCraft, GridCraft,
DeckCraft, PdfCraft and FigCraft — plus three new native crafts: Draw (drawio),
Markdown and Typeset.** The upstream apps are linked **unchanged** from git submodules
under [`upstream/`](upstream) and render their own complete, real UIs (their
chrome, ribbons, toolbars, panels and status bars — pixel-for-pixel theirs).

## Why this is "same same"

- The UI you get is **the upstream UI code itself** (`crates/ui-egui` of each
  repo — WordCraft's alone is 54k lines across ~60 modules), not a re-creation.
- All crafts share storytold's own UI foundation
  [`craft-ui`](https://github.com/storytold/craft-libs) (buttons, docking,
  icons, tabs, theme) — patched to one local checkout so the suite links a
  single copy.
- Each craft keeps its own engine, model and file readers; opening a `.docx`
  runs WordCraft's real `file.open` command, a `.pptx` runs DeckCraft's, etc.

## Layout

```
doccraft-suite/
├── app/                    the DocCraft shell: launcher, hub strip, file routing
├── crates/
│   ├── drawio/             native drawio craft (mxGraph model + XML + editor)
│   ├── md/                 native Markdown craft (parser + live split preview)
│   └── typeset/            native typesetting craft (layout engine + PDF export)
├── upstream/               git submodules — the real repos, updatable
│   ├── craft-libs/         shared craft-ui widgets
│   ├── wordcraft/          v0.8.0   (Word)     ~60% ready, ribbon catalog 97%
│   ├── gridcraft/          v0.8.0   (Excel)    breadth ~75%, functions 98.1%
│   ├── deckcraft/          v0.8.0   (PPT)      breadth 84%, ready ~55%
│   └── pdfcraft/           v0.9.0   (Acrobat)  ready ~50%
├── scripts/update-crafts.sh   pull every upstream repo to latest + re-pin
├── CRAFT-STATUS.md         versions + honest readiness per repo (from their ROADMAPs)
└── PROGRESS.md             what was built and how it links
```

## Build & run

```bash
cargo build --profile fast      # 2-core-friendly profile (opt 1, no LTO)
./target/fast/doccraft          # launcher opens; pick a craft
./target/fast/doccraft report.docx plan.drawio notes.md   # open straight into a craft
```

## Updating the upstream crafts

```bash
scripts/update-crafts.sh              # all five repos → latest main
scripts/update-crafts.sh wordcraft    # just one
cargo build --profile fast            # rebuild the suite against the new pins
```

Each `upstream/<repo>` is a normal git submodule: `git submodule status`
prints the pinned SHA, `git -C upstream/wordcraft checkout vX.Y.Z` pins a
release, and `git add upstream/wordcraft` records it.

## Command line

```
doccraft [--sample] [files…]
```

Files route by extension: `.docx .doc .odt` → Writer · `.xlsx .csv …` →
Sheets · `.pptx` → Slides · `.pdf` → Reader · `.drawio` → Draw · `.md` →
Markdown · `.typ .tex` → Typeset.

## Licenses

Upstream crafts: MIT OR Apache-2.0 (see each repo). DocCraft glue: MIT.
