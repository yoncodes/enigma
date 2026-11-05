#!/usr/bin/env python3
"""Generate the version-locked test hook payload in an isolated directory."""
from pathlib import Path
import sys

destination = Path(sys.argv[1]).resolve()
original = (destination / "original.luao").read_bytes()
candidate = (destination / "candidate.luao").read_bytes()
settings_original = (destination / "settings_original.luao").read_bytes()
settings_candidate = (destination / "settings_candidate.luao").read_bytes()

def array(name, data):
    lines = [f"static const unsigned char {name}[] = {{\n"]
    for offset in range(0, len(data), 16):
        lines.append("  " + ",".join(f"0x{byte:02x}" for byte in data[offset:offset + 16]) + ",\n")
    lines.append("};\n")
    return "".join(lines)

(destination / "payload.h").write_text(array("original_module", original) + array("candidate_module", candidate)
    + array("original_settings", settings_original) + array("candidate_settings", settings_candidate))
print(f"Prepared version-locked test hook payload in {destination}")
