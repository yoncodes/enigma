#!/usr/bin/env python3
"""Build an isolated research bundle. This tool never installs game files.

The record layout follows the publicly documented re1999-tools extractor:
https://github.com/hax0r31337/re1999-tools/tree/main/lua-bundles
The 48-byte prefix and all records except FightSkillComp and the graphics view
are preserved byte for byte. This candidate is only a build/test intermediate.
"""

import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import struct
import tempfile

from lupa.luajit21 import LuaRuntime


MODULE_PATH = "modules/logic/fight/entity/comp/FightSkillComp.lua"
MODULE_TAG = hashlib.md5(MODULE_PATH.encode("utf-8")).hexdigest().encode("ascii")
SETTINGS_PATH = "modules/logic/settings/view/SettingsGraphicsView.lua"
SETTINGS_TAG = hashlib.md5(SETTINGS_PATH.encode("utf-8")).hexdigest().encode("ascii")
ROOT = Path(__file__).resolve().parents[1]


def entries(bundle):
    if len(bundle) < 48:
        raise ValueError("bundle is shorter than its 48-byte header")
    position = 48
    seen = set()
    while position < len(bundle):
        begin = position
        tag_length = 0
        for _ in range(5):
            if position >= len(bundle):
                raise ValueError("truncated tag length")
            byte = bundle[position]
            position += 1
            tag_length = (tag_length << 7) | (byte & 0x7F)
            if byte < 0x80:
                break
        else:
            raise ValueError("invalid tag length")
        if not 1 <= tag_length <= 128 or position + tag_length + 4 > len(bundle):
            raise ValueError("invalid record tag")
        tag = bundle[position : position + tag_length]
        position += tag_length
        length = struct.unpack_from("<I", bundle, position)[0]
        position += 4
        end = position + length
        if end > len(bundle):
            raise ValueError("truncated record")
        if tag in seen:
            raise ValueError("duplicate module tag")
        seen.add(tag)
        yield tag, begin, position, end
        position = end


def unpack_record(bundle, tag, data_start, end):
    encrypted = bundle[data_start:end]
    packed = bytes(value ^ tag[index % len(tag)] for index, value in enumerate(encrypted))
    return gzip.decompress(packed)


def encode_varint(value):
    parts = [value & 0x7F]
    value >>= 7
    while value:
        parts.append((value & 0x7F) | 0x80)
        value >>= 7
    return bytes(reversed(parts))


def record(tag, data):
    packed = gzip.compress(data, mtime=0)
    encrypted = bytes(value ^ tag[index % len(tag)] for index, value in enumerate(packed))
    return encode_varint(len(tag)) + tag + struct.pack("<I", len(encrypted)) + encrypted


def lua_string(data):
    return b'"' + b"".join(b"\\%03d" % byte for byte in data) + b'"'


def compile_wrapper(original_bytecode, prototype_source, expose_global=True):
    if not original_bytecode.startswith(b"\x1bLJ\x02"):
        raise ValueError("target module is not LuaJIT 2.1 bytecode")
    source = (
        b"local chunk = assert(loadstring(" + lua_string(original_bytecode) + b", '@ultimate_skip.original'))\n"
        b"setfenv(chunk, getfenv(1))\n"
        b"local original = chunk(...)\n"
        b"local ok, prototype = pcall(function()\n" + prototype_source + b"\nend)\n"
        b"if ok and type(prototype)=='table' and type(prototype.install)=='function' then\n"
        b"  local installed, result = pcall(prototype.install, original)\n"
        # BootStarter rejects new globals through __newindex. Use the same raw
        # registration mechanism as setGlobal, or require() fails after install.
        + (b"  if installed then rawset(_G, 'UltimateSkipPrototype', result) end\n" if expose_global else b"")
        + b"  if not installed and logError then pcall(logError, 'Ultimate skip module could not be installed: '..tostring(result)) end\n"
        b"elseif logError then pcall(logError, 'Ultimate skip module could not be initialized: '..tostring(prototype)) end\n"
        b"return original\n"
    )
    lua = LuaRuntime(encoding=None, unpack_returned_tuples=True)
    compile_lua = lua.eval(
        b'function(source) local f,e=loadstring(source,"@ultimate_skip_wrapper.lua"); '
        b'if not f then error(e) end; return string.dump(f) end'
    )
    result = compile_lua(source)
    if result[:5] != original_bytecode[:5]:
        raise ValueError("compiled bytecode flags differ from the target module")
    return result


def build(bundle, prototype_source):
    records = list(entries(bundle))
    target_sources = {
        MODULE_TAG: (prototype_source, True),
        SETTINGS_TAG: ((ROOT / "src/settings_graphics.lua").read_bytes(), False),
    }
    rewritten = {}
    parts = [bundle[:48]]
    for tag, begin, data_start, end in records:
        if tag in target_sources:
            original = unpack_record(bundle, tag, data_start, end)
            source, expose_global = target_sources[tag]
            wrapper = compile_wrapper(original, source, expose_global)
            rewritten[tag] = (original, wrapper)
            parts.append(record(tag, wrapper))
        else:
            parts.append(bundle[begin:end])
    if set(rewritten) != set(target_sources):
        raise ValueError("one of the target Lua modules was not found")
    candidate = b"".join(parts)
    # Verify every original record outside the replacements is untouched.
    new_records = list(entries(candidate))
    if len(records) != len(new_records):
        raise ValueError("record count changed")
    for old, new in zip(records, new_records):
        if old[0] != new[0]:
            raise ValueError("record order changed")
        if old[0] not in rewritten and bundle[old[1] : old[3]] != candidate[new[1] : new[3]]:
            raise ValueError("unrelated record changed")
    for tag, (_, wrapper) in rewritten.items():
        new_target = next(item for item in new_records if item[0] == tag)
        if unpack_record(candidate, tag, new_target[2], new_target[3]) != wrapper:
            raise ValueError("replacement did not round-trip")
    original, wrapper = rewritten[MODULE_TAG]
    return candidate, original, wrapper, len(records)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", type=Path, help="original luabytes bundle")
    parser.add_argument("output", type=Path, help="new candidate file; never an installed asset")
    args = parser.parse_args()
    source_path = args.input.resolve(strict=True)
    output_path = args.output.resolve()
    if source_path == output_path or output_path.exists():
        parser.error("output must be a new file and cannot replace the input")
    if "StreamingAssets" in output_path.parts:
        parser.error("refusing to write into a game StreamingAssets directory")
    output_path.parent.mkdir(parents=True, exist_ok=True)
    source = source_path.read_bytes()
    candidate, original, wrapper, count = build(
        source, (ROOT / "src/ultimate_skip.lua").read_bytes()
    )
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=output_path.parent, delete=False) as tmp:
            temporary = Path(tmp.name)
            tmp.write(candidate)
        os.replace(temporary, output_path)
    finally:
        if temporary and temporary.exists():
            temporary.unlink()
    print(json.dumps({
        "input_sha256": hashlib.sha256(source).hexdigest(),
        "output_sha256": hashlib.sha256(candidate).hexdigest(),
        "module_original_sha256": hashlib.sha256(original).hexdigest(),
        "module_wrapper_sha256": hashlib.sha256(wrapper).hexdigest(),
        "records": count,
        "signature_and_manifest_verified": False,
        "mode_without_launcher_or_saved_setting": 0,
        "output": str(output_path),
    }, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
