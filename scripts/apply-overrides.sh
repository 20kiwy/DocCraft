#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# DocCraft system overrides — applied ON TOP of the upstream craft submodules.
#
# The user asked that each craft's own About window be folded into ONE system
# About: these edits retitle the per-craft About dialogs to name DocCraft as
# the hosting system. The edits are idempotent seds, so this script can be
# re-run at any time — in particular after `update-crafts.sh` pulls new
# upstream revisions (update-crafts.sh calls this script at the end).
#
# NOTE: applying this makes the submodules show a dirty worktree (that is the
# intended, documented trade-off). `git -C clone/<craft> checkout -- .`
# reverts to pristine upstream at any time.
# ─────────────────────────────────────────────────────────────────────────────
set -u
root="$(cd "$(dirname "$0")/.." && pwd)"
apply() { # file  sed-expr
  local f="$root/$1" expr="$2"
  [ -f "$f" ] || { echo "skip (missing): $1"; return; }
  if grep -q "DocCraft" "$f"; then echo "already applied: $1"; return 0; fi
  sed -i "$expr" "$f" && echo "patched: $1"
}

apply clone/wordcraft/crates/ui-egui/src/dialogs.rs \
  's|Dialog::About { .. } => "About WordCraft"|Dialog::About { .. } => "About DocCraft (WordCraft inside)"|'

apply clone/gridcraft/crates/ui-egui/src/dialogs.rs \
  's|Dialog::custom("about", "About GridCraft", json!({}))|Dialog::custom("about", "About DocCraft (GridCraft inside)", json!({}))|'

apply clone/deckcraft/crates/ui-egui/src/menus.rs \
  's|"About DeckCraft", "app.about"|"About DocCraft (DeckCraft inside)", "app.about"|'

apply clone/pdfcraft/crates/engine/src/commands.rs \
  's|"About PdfCraft", HELP|"About DocCraft (PdfCraft inside)", HELP|'

apply clone/figcraft/crates/ui-egui/src/toolbar.rs \
  's|(tr("About FigCraft"), "help.about")|(tr("About DocCraft (FigCraft inside)"), "help.about")|'

apply clone/figcraft/crates/ui-egui/src/dialogs.rs \
  's|window(tr("About FigCraft"), ctx)|window(tr("About DocCraft (FigCraft inside)"), ctx)|'

apply clone/figcraft/crates/engine/src/catalog.rs \
  's|m!("Help", "About FigCraft", "help.about")|m!("Help", "About DocCraft (FigCraft inside)", "help.about")|'

apply clone/figcraft/crates/engine/src/cmd/view.rs \
  's|"About FigCraft", "{}"|"About DocCraft (FigCraft inside)", "{}"|'

echo "DocCraft overrides applied."

# WordCraft: the title-bar Discord pill becomes a Website link (user request).
wd() { local f="$root/$1"; [ -f "$f" ] || return 0; grep -q "github.com/storytold" "$f" && { echo "already: $1"; return 0; }
  python3 - "$f" <<'PY'
import sys
p=sys.argv[1]; s=open(p).read()
s=s.replace('https://discord.gg/artcraft','https://github.com/storytold')
s=s.replace('tl!("Discord"), medium(11.5)','tl!("Website"), medium(11.5)')
s=s.replace('Join the ArtCraft community on Discord','Open the storytold website')
s=s.replace('"Community\\nDiscord", "ui.discord"','"Website\\nstorytold", "ui.discord"')
s=s.replace('c("Community Discord", "ui.discord")','c("Website", "ui.discord")')
open(p,'w').write(s)
PY
  echo "patched: $1"; }
wd clone/wordcraft/crates/ui-egui/src/lib.rs
wd clone/wordcraft/crates/ui-egui/src/chrome.rs
wd clone/wordcraft/crates/ui-egui/src/ribbon.rs
wd clone/wordcraft/crates/ui-egui/src/keytips.rs
