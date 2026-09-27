#!/usr/bin/env bash
# GitHub Copilot; updated 2026-09-27T22:25:02Z
set -euo pipefail

script_path="${1:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/generate-commit-activity.py}"
temp_dir="$(mktemp -d)"
trap 'rm -rf "$temp_dir"' EXIT

repo="$temp_dir/repo"
mkdir -p "$repo"
git -C "$repo" init -q -b main
git -C "$repo" config user.name "Commit chart test"
git -C "$repo" config user.email "commit-chart@example.invalid"

printf 'first\n' > "$repo/file"
git -C "$repo" add file
GIT_AUTHOR_DATE=2026-09-28T08:00:00Z GIT_COMMITTER_DATE=2026-09-28T08:00:00Z \
  git -C "$repo" commit -q -m "first commit"
printf 'second\n' >> "$repo/file"
git -C "$repo" add file
GIT_AUTHOR_DATE=2026-09-28T09:00:00Z GIT_COMMITTER_DATE=2026-09-28T09:00:00Z \
  git -C "$repo" commit -q -m "second commit"
printf 'single day\n' >> "$repo/file"
git -C "$repo" add file
GIT_AUTHOR_DATE=2026-09-27T09:00:00Z GIT_COMMITTER_DATE=2026-09-27T09:00:00Z \
  git -C "$repo" commit -q -m "single-day commit"
printf 'boundary\n' >> "$repo/file"
git -C "$repo" add file
GIT_AUTHOR_DATE=2025-09-28T08:00:00Z GIT_COMMITTER_DATE=2025-09-28T08:00:00Z \
  git -C "$repo" commit -q -m "boundary commit"
printf 'old\n' >> "$repo/file"
git -C "$repo" add file
GIT_AUTHOR_DATE=2024-01-01T08:00:00Z GIT_COMMITTER_DATE=2024-01-01T08:00:00Z \
  git -C "$repo" commit -q -m "old commit"

python3 "$script_path" --repo "$repo" --output "$temp_dir/activity.svg" --today 2026-09-28
grep -q 'data-date="2026-09-28" data-count="2"' "$temp_dir/activity.svg"
grep -q 'fill="#9be9a8" data-date="2026-09-27" data-count="1"' "$temp_dir/activity.svg"
if grep -q 'data-date="2024-01-01"' "$temp_dir/activity.svg"; then
  printf 'Old commit unexpectedly appeared in the one-year chart.\n' >&2
  exit 1
fi
if grep -q 'data-date="2025-09-28"' "$temp_dir/activity.svg"; then
  printf 'Commit before the one-year window unexpectedly appeared.\n' >&2
  exit 1
fi