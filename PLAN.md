# DocCraft 3.0 — Plan

## Mission (user's words, decoded)

> "copy all crates from all the system to one crates … make sure u add git so
> i can update all crate data well … check what percent each is right now and
> the version … copy full ui so to be same same respect its code so to
> integrate all 4 and add the new other drawio, md and typset native like
> those other 4 … toolbar or layout are same as what the craft software have"

## Principles

1. **The real code, not a look-alike.** The UI of each craft is the upstream
   `crates/ui-egui` itself, linked unmodified. No re-implementations.
2. **Updatable per repo.** Every upstream craft is a git submodule under
   `upstream/`; one script re-pins all of them to latest and the suite
   rebuilds against the new source.
3. **Honest accounting.** Versions and readiness percentages are quoted from
   each upstream ROADMAP verbatim (CRAFT-STATUS.md), never invented.
4. **New crafts follow the real pattern.** drawio / md / typeset are built
   like storytold crates: typed model → engine → egui front end, and they
   live in `crates/` next to the shell, not bolted inside it.

## Milestones

| # | Milestone | Done when |
|---|---|---|
| M1 | Recon | 5 repos cloned; versions, readiness %, embed surfaces recorded |
| M2 | Workspace | doccraft-suite workspace + 5 submodules + `[patch]` craft-ui unification |
| M3 | Shell | launcher cards, bottom hub strip, extension routing, lazy mounting |
| M4 | Embeds | WordApp / SheetApp / SlideApp / PdfCraftApp construct and render inside the host |
| M5 | Native crafts | drawio editor, markdown editor, typeset engine compile and run |
| M6 | Build | `cargo build --profile fast` green on 2 cores |
| M7 | Evidence | Xvfb headless screenshots of launcher + all 7 craft views → `public/` |
| M8 | Delivery | ZIP (code + docs) → `download/`, source mirror → `public/doccraft-suite-source/` |

## Known risks & mitigations

| Risk | Mitigation |
|---|---|
| aws-lc-sys (pdfcraft RSA) needs cmake | cmake 3.31.6 extracted to `/home/z/local-libs` from Debian debs; PATH set at build time |
| GTK3 absent (rfd default?) | rfd 0.17 defaults are already `xdg-portal + wayland` — pure Rust, nothing to do |
| craft-ui pinned at two different revs by wordcraft/pdfcraft | `[patch."https://github.com/storytold/craft-libs"]` routes both to one local checkout |
| eframe/wgpu needs GL in sandbox | `WGPU_BACKEND=gl` + EGL libs from `/home/z/local-libs` (proven recipe from DocCraft 2.x) |
| Build time (≈843k upstream lines) | `fast` profile (opt 1, 256 CGUs, incremental, no LTO); per-crate caching resumes across interruptions |

## Out of scope (explicitly)

- Changing upstream code (only additions live in `app/` and `crates/`).
- MCP control for the new crafts (upstream MCP crates remain compiled-in via
  the ui-egui deps; a unified DocCraft control plane can come later).
