#!/usr/bin/env python3
"""Validate the ordered OSC 633 frames captured by the spike harness."""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path


FRAME_RE = re.compile(r"^\d+:\s?(.*)$")


def read_frames(path: Path) -> list[str]:
    frames: list[str] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        match = FRAME_RE.match(line)
        if match:
            frames.append(match.group(1))
    return frames


def analyze(frames: list[str], nonce: str | None) -> dict[str, object]:
    commands: list[str] = []
    lifecycle_errors: list[str] = []
    current: list[str] = []
    for frame in frames:
        kind = frame.split(";", 1)[0]
        if kind == "E":
            # PSReadLine may emit an intermediate E/C pair for each Enter
            # while a multiline buffer is being assembled. Keep the latest
            # complete buffer; D closes the command that actually ran.
            current = [frame]
        elif kind == "C":
            if not current:
                lifecycle_errors.append("C appeared without E")
            else:
                current.append(frame)
        elif kind == "D":
            if not current:
                continue
            current.append(frame)
            commands.append(";".join(current))
            current = []
    if current:
        lifecycle_errors.append("unterminated E/C/D sequence")

    nonce_errors = []
    if nonce is not None:
        for frame in frames:
            if frame.startswith("E;") and not frame.endswith(";" + nonce):
                nonce_errors.append(frame)

    result = {
        "frames": len(frames),
        "commands": len(commands),
        "prompt_starts": frames.count("A"),
        "prompt_ends": frames.count("B"),
        "cwd_markers": sum(frame.startswith("P;Cwd=") for frame in frames),
        "lifecycle_errors": lifecycle_errors,
        "nonce_errors": nonce_errors,
        "ok": bool(frames) and not lifecycle_errors and not nonce_errors,
    }
    return result


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--frames", type=Path, default=Path("osc633.frames.txt"))
    parser.add_argument("--nonce")
    args = parser.parse_args()
    result = analyze(read_frames(args.frames), args.nonce)
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0 if result["ok"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
