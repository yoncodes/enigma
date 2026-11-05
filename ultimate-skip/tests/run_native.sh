#!/usr/bin/env bash
# Rebuild and exercise the lab-only hook using this workspace's client DLL.
set -euo pipefail

project_dir="$(cd "$(dirname "$0")/.." && pwd)"
build_root=/opt/data/private/1999/enigma-v65-remote-build-20261031
lua_path=/tmp/r1999-ult-lua-runtime
test_dir="$(mktemp -d /tmp/r1999-ultimate-test-XXXXXX)"
trap 'rm -rf "$test_dir"' EXIT

cd "$project_dir"
PYTHONPATH="$lua_path" python3 tests/prepare_native.py "$test_dir"
PYTHONPATH="$lua_path" python3 tests/prepare_hook.py "$test_dir"

clang="$build_root/llvm/bin/clang"
kernel32="$build_root/cache/cargo-xwin/xwin/sdk/lib/um/x86_64/kernel32.Lib"
export LD_LIBRARY_PATH="$build_root/compat/lib/x86_64-linux-gnu"
"$clang" --target=x86_64-pc-windows-msvc -shared -nostdlib -fuse-ld=lld \
    -fno-stack-protector -I"$test_dir" src/native_lua_hook.c "$kernel32" \
    -Wl,/entry:DllMain -o "$test_dir/ultimate_hook.dll"
"$clang" --target=x86_64-pc-windows-msvc -nostdlib -fuse-ld=lld \
    -fno-stack-protector tests/native_load.c "$kernel32" \
    -Wl,/entry:mainCRTStartup,/subsystem:console -o "$test_dir/normal_test.exe"
"$clang" --target=x86_64-pc-windows-msvc -DTEST_RUNTIME_HOOK -nostdlib \
    -fuse-ld=lld -fno-stack-protector tests/native_load.c "$kernel32" \
    -Wl,/entry:mainCRTStartup,/subsystem:console -o "$test_dir/hook_test.exe"

export WINEDEBUG=-all
export WINEPREFIX=/tmp/r1999-ultimate-native-prefix
export LD_LIBRARY_PATH="$build_root/client-headless-20261102/runtime/usr/lib/x86_64-linux-gnu"
wine="$build_root/client-headless-20261102/wine10/opt/wine-stable/bin/wine64"
cd "$test_dir"
"$wine" normal_test.exe
"$wine" hook_test.exe
