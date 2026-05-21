#!/usr/bin/env bash
# Helper: remove tracked target/.rustc_info.json from the repository and add to .gitignore
# Run from repo root.

set -euo pipefail

FILE=target/.rustc_info.json
if git ls-files --error-unmatch "$FILE" >/dev/null 2>&1; then
    echo "Removing tracked $FILE from git index..."
    git rm --cached "$FILE"
    git commit -m "chore: stop tracking target/.rustc_info.json"
    echo "File removed from index. Pushed change required to update remote."
else
    echo "$FILE is not tracked by git. Nothing to do."
fi

echo "Ensure .gitignore contains an entry to ignore this file (target/ is recommended)."
