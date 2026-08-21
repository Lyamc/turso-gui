"""Render screenshots/release-compare.png from screenshots/release-bench.json."""
from __future__ import annotations

import json
from pathlib import Path

import matplotlib.pyplot as plt
import numpy as np

ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "screenshots" / "release-bench.json"
OUT = ROOT / "screenshots" / "release-compare.png"


def main() -> None:
    rows = json.loads(DATA.read_text(encoding="utf-8-sig"))
    names = [r["name"] for r in rows]
    sizes = np.array([r["file_bytes"] / (1024 * 1024) for r in rows])
    times = np.array([r["open_close_ms"] / 1000.0 for r in rows])
    mems = np.array([r["peak_working_set"] / (1024 * 1024) for r in rows])
    cycles = rows[0]["cycles"] if rows else 5

    plt.rcParams.update(
        {
            "font.family": "Segoe UI",
            "axes.titlesize": 12,
            "axes.labelsize": 10,
            "xtick.labelsize": 10,
            "ytick.labelsize": 9,
            "figure.facecolor": "white",
        }
    )
    fig, axes = plt.subplots(1, 3, figsize=(12.8, 5.0), constrained_layout=True)
    fig.suptitle(
        f"Release binaries — size and {cycles}× open/close",
        fontsize=14,
        fontweight="semibold",
    )

    colors = ["#1a66cc", "#2a9d8f", "#e76f51"]
    specs = [
        (axes[0], sizes, "File size (MiB)", colors[0], "{:.2f}"),
        (axes[1], times, f"Time for {cycles} open/close (s)", colors[1], "{:.2f}"),
        (axes[2], mems, "Peak working set (MiB)", colors[2], "{:.0f}"),
    ]

    x = np.arange(len(names))
    for ax, values, title, color, fmt in specs:
        bars = ax.bar(x, values, color=color, width=0.72, zorder=3)
        ax.set_title(title)
        ax.set_xticks(x, names)
        ax.set_ylabel(title.split("(")[-1].rstrip(")") if "(" in title else "")
        ax.grid(axis="y", linestyle=":", alpha=0.6, zorder=0)
        ax.set_axisbelow(True)
        ymax = max(values) * 1.28 if max(values) > 0 else 1
        ax.set_ylim(0, ymax)
        rel = values / values.min() if values.min() > 0 else values
        for bar, val, r in zip(bars, values, rel):
            ax.annotate(
                f"{fmt.format(val)}\n({r:.2f}×)",
                xy=(bar.get_x() + bar.get_width() / 2, bar.get_height()),
                xytext=(0, 4),
                textcoords="offset points",
                ha="center",
                va="bottom",
                fontsize=8,
            )

    fig.savefig(OUT, dpi=140, bbox_inches="tight")
    print(f"wrote {OUT}")


if __name__ == "__main__":
    main()
