# CRAFT STATUS — what each upstream craft is, right now

Checked from each repo's own ROADMAP.md and Cargo.toml at integration time
(submodule SHAs below). Update with `scripts/update-crafts.sh`.

| Craft | Repo | Version | Submodule HEAD | Stage | Honest readiness | Measured numbers |
|---|---|---|---|---|---|---|
| WordCraft (Writer) | github.com/storytold/wordcraft | 0.8.0 | `4a251d4` (main) | alpha | **~60%** ready for real work | ribbon catalog 487/502 (**97.0%**), feature breadth ~80%, 433 commands, 855 tests, ~88.7k lines (their core) |
| GridCraft (Sheets) | github.com/storytold/gridcraft | 0.8.0 | `c30fa3f` (v0.8.0-4) | alpha | breadth **~75%** | ribbon 274/290 (94.5%), functions 513/523 (98.1%) |
| DeckCraft (Slides) | github.com/storytold/deckcraft | 0.8.0 | `22db926` (**v0.8.0** tag) | alpha | ready **~55%** | breadth 84% after `cargo xtask parity` |
| PdfCraft (Reader) | github.com/storytold/pdfcraft | 0.9.0 | `a12fe8f` (v0.9.0-38) | alpha | ready **~50%** (weighted 49%) | Acrobat Pro DC target, OCR via rten, XFA, signing |
| FigCraft (Design) | github.com/storytold/figcraft | 0.7.0 | `d8ff144` (v0.7.0-104) | alpha | menu breadth **100%** (212/212), feature breadth **~85%** (core editor ~94%), ready **~53%** | Figma Design (desktop 126.10.7) target; frames, vectors, text, auto layout, components, styles, variables, prototyping, Dev Mode |
| craft-libs | github.com/storytold/craft-libs | 0.1.0 | `82a24b0` (main) | shared | — | `craft-ui`: buttons, docking, icons, layout, printing, tabs, theme — the UI foundation all crafts share |
| Draw (drawio) | doccraft-suite/crates/drawio | 3.4.0 | this repo | native | new craft | full GraphEditor UI (Arrange/Help menus, scratchpad, drag-from-palette, Diagram tab, page ops), mxGraph XML round-trip, drawio-exact connections per DRAWIO-SPEC.md, SVG/PNG/PDF export |
| Markdown | doccraft-suite/crates/md | 3.4.0 | this repo | native | new craft | CommonMark subset, split live preview on page cards, theme toggle |
| Typeset | doccraft-suite/crates/typeset | 3.4.0 | this repo | native | new craft | markup → justified paginated A4 → PDF (Type1 base-14), paginated page cards |
| Notes | doccraft-suite/crates/notes | 3.4.0 | this repo | native | new craft | multi-page notebook, styled live preview, settings, .note JSON save/load |

Notes
- Every readiness number above is copied from the upstream repo's own
  ROADMAP.md "headline numbers" — not our estimate. The crafts are **not 100%
  complete** (they say so themselves); embedding them gives DocCraft exactly
  the same capability set as each upstream at the pinned commit.
- `git submodule status` re-prints the pinned SHAs at any time.

## 3.2 additions
| system | version | source | readiness |
|---|---|---|---|
| Notes craft | 3.2.0 | new | native notebook |
| craft-launcher (ArtCraft Launcher) | 0.5.1 | github.com/storytold/craft-launcher | UI/theme/motion reference — copied into the DocCraft shell |
| drawio JS (jgraph/drawio v32.5.5 + mxGraph) | — | github.com/jgraph/drawio | connection/arrow/export spec extracted → DRAWIO-SPEC.md → implemented natively |
| Windows target | — | cargo-xwin cross-build | DocCraft.exe x64 + installer + 27 file associations |

## 3.4 additions
| system | version | source | readiness |
|---|---|---|---|
| doccraft-theme (shared design tokens) | 3.4.0 | new | FigCraft UI3 light/dark tokens (ACCENT #0D99FF, radius 5/13, page shadows) — every native craft paints from it; ☀/☾ toggle per craft |
| Draw → full GraphEditor | 3.4.0 | upgraded | Arrange + Help menus, File New/Save As/Close, multi-open shape libraries, Scratchpad (click/✕, auto-add on copy), drag shapes from palette onto canvas, page-tab context ops (rename/duplicate/delete/insert), Diagram tab with page setup, toolbar zoom menu |
| Markdown / Typeset restyle | 3.4.0 | upgraded | UI3 tokens, page-card preview, DocCraft Blue default accent, Light/Dark in settings |
| Notes full parity | 3.4.0 | upgraded | glyph ribbon (CLIPBOARD/FONT/PARAGRAPH/EDITING/VIEWS), styled markdown preview on page card, settings + accent picker, .note JSON save/load (Ctrl+S), markdown-marker toolbar actions |
| Figma files | 3.4.0 | routing | `.figma` opens via Design (FigCraft) — launcher routing + Windows associations (27 types) |
