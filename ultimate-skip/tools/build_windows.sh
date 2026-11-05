#!/usr/bin/env bash
set -euo pipefail
project_dir="$(cd "$(dirname "$0")/.." && pwd)"
build_root=/opt/data/private/1999/enigma-v65-remote-build-20261031
sdk="$build_root/cache/cargo-xwin/xwin"
clang="$build_root/llvm/bin/clang"
kernel32="$sdk/sdk/lib/um/x86_64/kernel32.Lib"
release_name="${R1999_RELEASE_NAME:-R1999-ultimate-skip-test}"
output="$project_dir/dist/$release_name"
stage="$(mktemp -d /tmp/r1999-release-build-XXXXXX)"
trap 'rm -rf "$stage"' EXIT
mkdir -p "$output"
cd "$project_dir"
PYTHONPATH=/tmp/r1999-ult-lua-runtime python3 tests/prepare_native.py "$stage"
PYTHONPATH=/tmp/r1999-ult-lua-runtime python3 tests/prepare_hook.py "$stage"
export LD_LIBRARY_PATH="$build_root/compat/lib/x86_64-linux-gnu"
inc=(-isystem "$sdk/crt/include" -isystem "$sdk/sdk/include/ucrt" -isystem "$sdk/sdk/include/shared" -isystem "$sdk/sdk/include/um")
flags=(--target=x86_64-pc-windows-msvc -fuse-ld=lld -nostdlib -fno-builtin -fno-stack-protector)
"$clang" "${flags[@]}" "${inc[@]}" src/windows_launcher.c "$kernel32" \
    -Wl,/entry:mainCRTStartup,/subsystem:console,/manifest:embed,"/manifestuac:level='requireAdministrator' uiAccess='false'" \
    -o "$output/ultimate-skip.exe"
"$clang" "${flags[@]}" "${inc[@]}" -shared -DULTIMATE_AUTO_INSTALL -I"$stage" \
    src/native_lua_hook.c src/auto_attach.c "$kernel32" -Wl,/entry:DllMain \
    -o "$output/ultimate-skip-hook.dll"
printf "Built Windows launcher and hook in %s\n" "$output"
