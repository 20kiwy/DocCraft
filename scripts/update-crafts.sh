#!/usr/bin/env bash
# Update every upstream storytold craft to the latest main (or its release tag).
#   scripts/update-crafts.sh            # update all five
#   scripts/update-crafts.sh wordcraft  # update just one
set -euo pipefail
cd "$(dirname "$0")/.."

repos=(craft-libs wordcraft pdfcraft gridcraft deckcraft)
if [ $# -gt 0 ]; then repos=("$@"); fi

for r in "${repos[@]}"; do
  echo "── updating clone/$r"
  git -C "clone/$r" fetch --tags --prune origin
  default=$(git -C "clone/$r" rev-parse --abbrev-ref origin/HEAD 2>/dev/null | sed 's|origin/||' || echo main)
  git -C "clone/$r" checkout -q "$default" && git -C "clone/$r" pull -q --ff-only || true
  git add "clone/$r"
done

echo
echo "New upstream versions:"
for r in "${repos[@]}"; do
  v=$(grep -m1 '^version' "clone/$r/Cargo.toml" 2>/dev/null | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' || echo '?')
  h=$(git -C "clone/$r" rev-parse --short HEAD)
  printf '  %-12s v%s @ %s\n' "$r" "$v" "$h"
done
echo "Done. Now rebuild:  cargo build --profile fast"
