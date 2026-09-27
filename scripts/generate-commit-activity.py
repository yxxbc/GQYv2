# GitHub Copilot; updated 2026-09-27T22:25:02Z
"""Generate an SVG calendar from commits reachable from HEAD."""

from __future__ import annotations

import argparse
import html
import subprocess
from collections import Counter
from datetime import date, timedelta
from pathlib import Path


COLORS = ("#ebedf0", "#9be9a8", "#40c463", "#30a14e", "#216e39")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path.cwd())
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--today", type=date.fromisoformat, default=date.today())
    args = parser.parse_args()

    end = args.today
    window_start = end - timedelta(days=364)
    start = window_start - timedelta(days=window_start.weekday())
    result = subprocess.run(
        ["git", "-C", str(args.repo), "log", "HEAD", "--format=%cs"],
        check=True,
        capture_output=True,
        text=True,
    )
    counts = Counter(
        commit_date
        for line in result.stdout.splitlines()
        if (commit_date := date.fromisoformat(line)) >= window_start and commit_date <= end
    )

    weeks = (end - start).days // 7 + 1
    cell_size = 12
    gap = 3
    left = 38
    top = 42
    width = left + weeks * (cell_size + gap) + 12
    height = top + 7 * (cell_size + gap) + 18
    total = sum(counts.values())
    parts = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" '
        f'viewBox="0 0 {width} {height}" '
        'role="img" aria-labelledby="title description">',
        '<title id="title">GQYv2 commit activity</title>',
        '<desc id="description">Daily commit counts on the default branch over the last year.</desc>',
        '<g font-family="sans-serif" fill="#24292f">',
        '<text id="title-label" x="0" y="16" font-size="14" font-weight="600">Commit activity</text>',
        f'<text x="0" y="32" font-size="11" fill="#57606a">{total} commits in the last year</text>',
        '</g>',
    ]

    previous_month = None
    for week in range(weeks):
        current = start + timedelta(days=week * 7)
        if current <= end and current.month != previous_month:
            x = left + week * (cell_size + gap)
            parts.append(
                f'<text x="{x}" y="{top - 8}" font-family="sans-serif" '
                f'font-size="10" fill="#57606a">{html.escape(current.strftime("%b"))}</text>'
            )
            previous_month = current.month

        for weekday in range(7):
            cell_date = current + timedelta(days=weekday)
            if cell_date < window_start or cell_date > end:
                continue
            count = counts[cell_date]
            level = min(4, count.bit_length())
            x = left + week * (cell_size + gap)
            y = top + weekday * (cell_size + gap)
            label = f"{count} commits on {cell_date.isoformat()}"
            parts.append(
                f'<rect x="{x}" y="{y}" width="{cell_size}" height="{cell_size}" '
                f'rx="2" fill="{COLORS[level]}" data-date="{cell_date.isoformat()}" '
                f'data-count="{count}"><title>{label}</title></rect>'
            )

    parts.append('</svg>')
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text("\n".join(parts) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())