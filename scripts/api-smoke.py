#!/usr/bin/env python3
"""Reads every property of every @slintlu global from a running shell through IPC.

Usage: scripts/api-smoke.py path/to/slintlu
Fails if any documented property cannot be read, i.e. a global is not exported, a property is
misnamed or a native binding is missing.
"""
import pathlib
import re
import subprocess
import sys

slintlu = sys.argv[1] if len(sys.argv) > 1 else "slintlu"
api = (pathlib.Path(__file__).resolve().parent.parent / "docs" / "API.md").read_text()
globals_section = api.split("## Globals", 1)[1].split("\n## ", 1)[0]

targets = []
for block in globals_section.split("\n### ")[1:]:
    name = block.split()[0]
    for row in re.findall(r"^\| `([\w-]+)` \| `[^`]+` \| read", block, re.M):
        targets.append(f"{name}.{row}")

failures = []
for target in targets:
    result = subprocess.run([slintlu, "msg", "get", target], capture_output=True, text=True)
    if result.returncode != 0:
        failures.append(f"{target}: {result.stderr.strip()}")

print(f"read {len(targets) - len(failures)}/{len(targets)} global properties")
if failures:
    print("\n".join(failures))
    sys.exit(1)
