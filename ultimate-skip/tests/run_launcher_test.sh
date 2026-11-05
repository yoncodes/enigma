#!/usr/bin/env bash
set -euo pipefail
project_dir="$(cd "$(dirname "$0")/.." && pwd)"
build_root=/opt/data/private/1999/enigma-v65-remote-build-20261031
sdk="$build_root/cache/cargo-xwin/xwin"
clang="$build_root/llvm/bin/clang"
kernel32="$sdk/sdk/lib/um/x86_64/kernel32.Lib"
test_dir="$(mktemp -d /tmp/r1999-launcher-test-XXXXXX)"
trap 'rm -rf "$test_dir"' EXIT
mkdir -p "$test_dir/game" "$test_dir/launcher"
cd "$project_dir"
PYTHONPATH=/tmp/r1999-ult-lua-runtime python3 tests/prepare_native.py "$test_dir/game"
PYTHONPATH=/tmp/r1999-ult-lua-runtime python3 tests/prepare_hook.py "$test_dir/game"
export LD_LIBRARY_PATH="$build_root/compat/lib/x86_64-linux-gnu"
inc=(-isystem "$sdk/crt/include" -isystem "$sdk/sdk/include/ucrt" -isystem "$sdk/sdk/include/shared" -isystem "$sdk/sdk/include/um")
flags=(--target=x86_64-pc-windows-msvc -fuse-ld=lld -nostdlib -fno-builtin -fno-stack-protector)
if [[ "${1:-}" == "--release-artifact" ]]; then
    python3 - "${R1999_RELEASE_ZIP:-$project_dir/dist/R1999-ultimate-skip-test.zip}" "$test_dir/launcher" <<'PY'
import sys, zipfile
with zipfile.ZipFile(sys.argv[1]) as package: package.extractall(sys.argv[2])
PY
else
    "$clang" "${flags[@]}" "${inc[@]}" -DULTIMATE_TEST src/windows_launcher.c "$kernel32" \
        -Wl,/entry:mainCRTStartup,/subsystem:console -o "$test_dir/launcher/ultimate-skip.exe"
    "$clang" "${flags[@]}" "${inc[@]}" -shared -DULTIMATE_AUTO_INSTALL -I"$test_dir/game" \
        src/native_lua_hook.c src/auto_attach.c "$kernel32" -Wl,/entry:DllMain \
        -o "$test_dir/launcher/ultimate-skip-hook.dll"
fi
"$clang" "${flags[@]}" "${inc[@]}" tests/mock_launcher.c "$kernel32" \
    -Wl,/entry:mainCRTStartup,/subsystem:console -o "$test_dir/game/mock_launcher.exe"
if [[ "${1:-}" == "--incompatible" ]]; then
    "$clang" "${flags[@]}" "${inc[@]}" -shared tests/mock_incompatible.c "$kernel32" \
        -Wl,/entry:DllMain -o "$test_dir/game/tolua.dll"
    "$clang" "${flags[@]}" "${inc[@]}" tests/mock_incompatible_game.c "$kernel32" \
        -Wl,/entry:mainCRTStartup,/subsystem:console -o "$test_dir/game/reverse1999.exe"
else
    "$clang" "${flags[@]}" -DTEST_AUTO_INJECTION tests/native_load.c "$kernel32" \
        -Wl,/entry:mainCRTStartup,/subsystem:console -o "$test_dir/game/reverse1999.exe"
fi
python3 - "$test_dir" "${1:-}" <<'PY'
from pathlib import Path
import sys
root = Path(sys.argv[1])
winpath = lambda p: 'Z:' + str(p).replace('/', '\\')
text = ('[paths]\nlauncher=' + winpath(root/'game/mock_launcher.exe')
        + '\ngame_exe=' + winpath(root/'game/reverse1999.exe')
        + '\n[settings]\ndefault_mode=2\n')
(root/'launcher/ultimate-skip.ini').write_text(text, encoding='utf-16')
PY
export WINEDEBUG=-all
export WINEPREFIX=/tmp/r1999-ultimate-native-prefix
export LD_LIBRARY_PATH="$build_root/client-headless-20261102/runtime/usr/lib/x86_64-linux-gnu"
wine="$build_root/client-headless-20261102/wine10/opt/wine-stable/bin/wine64"
cd "$test_dir/launcher"
timeout 20 "$wine" ultimate-skip.exe > "$test_dir/output.log" 2>&1
for _ in $(seq 1 100); do
    if [[ "${1:-}" == "--incompatible" ]]; then
        grep -q 'incompatible runtime left running' "$test_dir/game/mock-game.log" 2>/dev/null && break
    else
        grep -q 'runtime interception and eleven effect/camera tests passed' "$test_dir/game/mock-game.log" 2>/dev/null && break
    fi
    sleep 0.2
done
cat "$test_dir/output.log"
cat "$test_dir/game/mock-game.log"
cat "$test_dir/launcher/ultimate-skip.log"
if [[ "${1:-}" == "--incompatible" ]]; then
    grep -q 'runtime version mismatch; hook not installed' "$test_dir/launcher/ultimate-skip.log"
    grep -q 'incompatible runtime left running' "$test_dir/game/mock-game.log"
else
    grep -q 'runtime hook installed' "$test_dir/launcher/ultimate-skip.log"
    grep -q 'battle module replaced' "$test_dir/launcher/ultimate-skip.log"
    grep -q 'settings module replaced' "$test_dir/launcher/ultimate-skip.log"
    grep -q 'BATTLE_TRACE prototype 0.8.0' "$test_dir/launcher/ultimate-skip.log"
    grep -q 'BATTLE_TIMELINE skill=42 .*action=play' "$test_dir/launcher/ultimate-skip.log"
    grep -q 'BATTLE_TIMELINE skill=31495151 .*unique_2 .*action=skip' "$test_dir/launcher/ultimate-skip.log"
    grep -q 'BATTLE_EXEC skill=31495151 action=skip' "$test_dir/launcher/ultimate-skip.log"
    grep -q 'game global guard regression passed' "$test_dir/game/mock-game.log"
    grep -q 'runtime interception and eleven effect/camera tests passed' "$test_dir/game/mock-game.log"
fi
