/* Starts the existing launcher and loads the experimental addon into the new
 * game process. It never overwrites the game or server files. */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <tlhelp32.h>

static SIZE_T wlength(const WCHAR *s) { SIZE_T n = 0; while (s[n]) n++; return n; }
static int copy(WCHAR *dst, SIZE_T cap, const WCHAR *src) {
    SIZE_T i = 0; while (src[i] && i + 1 < cap) { dst[i] = src[i]; i++; }
    dst[i] = 0; return !src[i];
}
static int append(WCHAR *dst, SIZE_T cap, const WCHAR *src) {
    SIZE_T len = wlength(dst); return len < cap && copy(dst + len, cap - len, src);
}
static void parent_dir(WCHAR *path) {
    SIZE_T len = wlength(path);
    while (len && path[len - 1] != L'\\' && path[len - 1] != L'/') len--;
    if (len) path[len - 1] = 0;
}
static int path_join(WCHAR *out, SIZE_T cap, const WCHAR *base, const WCHAR *name) {
    return copy(out, cap, base) && append(out, cap, L"\\") && append(out, cap, name);
}
static void say(const WCHAR *s) {
    HANDLE out = GetStdHandle(STD_OUTPUT_HANDLE);
    DWORD n;
    if (!WriteConsoleW(out, s, (DWORD)wlength(s), &n, NULL)) {
        static char data[8192];
        int bytes = WideCharToMultiByte(CP_UTF8, 0, s, -1, data, sizeof(data), NULL, NULL);
        if (bytes > 1) WriteFile(out, data, (DWORD)(bytes - 1), &n, NULL);
    }
}
static void finish_error(const WCHAR *message) {
    say(L"\r\n"); say(message); say(L"\r\n按回车键关闭窗口。\r\n");
    WCHAR input[4]; DWORD read;
    ReadConsoleW(GetStdHandle(STD_INPUT_HANDLE), input, 4, &read, NULL);
    ExitProcess(1);
}
static int full_path(const WCHAR *value, WCHAR *dest, DWORD size) {
    DWORD n = GetFullPathNameW(value, size, dest, NULL);
    return n && n < size;
}
static int is_file(const WCHAR *name) {
    DWORD attr = GetFileAttributesW(name);
    return attr != INVALID_FILE_ATTRIBUTES && !(attr & FILE_ATTRIBUTE_DIRECTORY);
}
static DWORD find_game(const WCHAR *game_path) {
    HANDLE snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
    if (snapshot == INVALID_HANDLE_VALUE) return 0;
    PROCESSENTRY32W entry;
    entry.dwSize = sizeof(entry);
    BOOL more = Process32FirstW(snapshot, &entry);
    DWORD found = 0;
    while (more) {
        if (lstrcmpiW(entry.szExeFile, L"reverse1999.exe") == 0) {
            HANDLE process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, entry.th32ProcessID);
            if (process) {
                static WCHAR path[2048]; DWORD len = 2048;
                if (QueryFullProcessImageNameW(process, 0, path, &len) && lstrcmpiW(path, game_path) == 0)
                    found = entry.th32ProcessID;
                CloseHandle(process);
                if (found) break;
            }
        }
        more = Process32NextW(snapshot, &entry);
    }
    CloseHandle(snapshot);
    return found;
}
static int inject(DWORD pid, const WCHAR *dll_path) {
    HANDLE process = OpenProcess(PROCESS_CREATE_THREAD | PROCESS_QUERY_INFORMATION | PROCESS_VM_OPERATION |
                                 PROCESS_VM_WRITE | PROCESS_VM_READ, FALSE, pid);
    if (!process) return 0;
    SIZE_T bytes = (wlength(dll_path) + 1) * sizeof(WCHAR);
    void *remote = VirtualAllocEx(process, NULL, bytes, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE);
    if (!remote) { CloseHandle(process); return 0; }
    SIZE_T written = 0;
    if (!WriteProcessMemory(process, remote, dll_path, bytes, &written) || written != bytes) {
        VirtualFreeEx(process, remote, 0, MEM_RELEASE); CloseHandle(process); return 0;
    }
    HMODULE kernel = GetModuleHandleW(L"kernel32.dll");
    LPTHREAD_START_ROUTINE load = (LPTHREAD_START_ROUTINE)GetProcAddress(kernel, "LoadLibraryW");
    HANDLE thread = load ? CreateRemoteThread(process, NULL, 0, load, remote, 0, NULL) : NULL;
    if (!thread) { VirtualFreeEx(process, remote, 0, MEM_RELEASE); CloseHandle(process); return 0; }
    DWORD wait = WaitForSingleObject(thread, 20000), result = 0;
    if (wait == WAIT_OBJECT_0) GetExitCodeThread(thread, &result);
    CloseHandle(thread);
    if (wait == WAIT_OBJECT_0) VirtualFreeEx(process, remote, 0, MEM_RELEASE);
    CloseHandle(process);
    return wait == WAIT_OBJECT_0 && result != 0;
}

void mainCRTStartup(void) {
    static WCHAR exe_dir[2048], ini[2048], dll[2048], log[2048], raw[2048], launcher[2048];
    static WCHAR launcher_dir[2048], game_raw[2048], game_path[2048], cmd[4096], old_ini[2048];
    DWORD n = GetModuleFileNameW(NULL, exe_dir, 2048);
    if (!n || n >= 2048) finish_error(L"无法读取程序目录。");
    parent_dir(exe_dir);
    if (!SetCurrentDirectoryW(exe_dir) || !path_join(ini, 2048, exe_dir, L"ultimate-skip.ini") ||
        !path_join(dll, 2048, exe_dir, L"ultimate-skip-hook.dll") ||
        !path_join(log, 2048, exe_dir, L"ultimate-skip.log")) finish_error(L"程序路径过长。");
    if (!is_file(ini) || !is_file(dll)) finish_error(L"缺少 ultimate-skip.ini 或 ultimate-skip-hook.dll，请完整解压。 ");
    GetPrivateProfileStringW(L"paths", L"launcher", L"", raw, 2048, ini);
    if (!raw[0] || !full_path(raw, launcher, 2048) || !is_file(launcher))
        finish_error(L"请在 ultimate-skip.ini 中填写现有启动器的完整路径。");
    copy(launcher_dir, 2048, launcher); parent_dir(launcher_dir);
    GetPrivateProfileStringW(L"paths", L"game_exe", L"", game_raw, 2048, ini);
    if (!game_raw[0]) {
        if (!path_join(old_ini, 2048, launcher_dir, L"r1999-launcher.ini")) finish_error(L"原启动器路径过长。");
        GetPrivateProfileStringW(L"paths", L"game_dir", L"", game_raw, 2048, old_ini);
        if (!game_raw[0]) finish_error(L"请在配置文件中填写 game_exe，或在原启动器配置中填写 game_dir。");
        if (!(game_raw[1] == L':' || (game_raw[0] == L'\\' && game_raw[1] == L'\\'))) {
            if (!path_join(raw, 2048, launcher_dir, game_raw)) finish_error(L"游戏路径过长。");
            copy(game_raw, 2048, raw);
        }
        if (!append(game_raw, 2048, L"\\reverse1999.exe")) finish_error(L"游戏路径过长。");
    }
    if (!full_path(game_raw, game_path, 2048) || !is_file(game_path))
        finish_error(L"找不到 reverse1999.exe，请检查 ultimate-skip.ini 里的 game_exe。");
    if (find_game(game_path)) finish_error(L"游戏已在运行，请先关闭游戏，再使用此启动器。 ");
    UINT mode = GetPrivateProfileIntW(L"settings", L"default_mode", 0, ini);
    if (mode != 0 && mode != 2) mode = 0;
    WCHAR mode_text[2] = {(WCHAR)(L'0' + mode), 0};
    SetEnvironmentVariableW(L"R1999_ULTIMATE_SKIP_DEFAULT_MODE", mode_text);
    SetEnvironmentVariableW(L"R1999_ULTIMATE_SKIP_LOG", log);
    SetEnvironmentVariableW(L"R1999_ULTIMATE_FIGHT_TRACE", L"1");
    HANDLE log_file = CreateFileW(log, FILE_APPEND_DATA, FILE_SHARE_READ | FILE_SHARE_WRITE,
                                  NULL, OPEN_ALWAYS, FILE_ATTRIBUTE_NORMAL, NULL);
    if (log_file != INVALID_HANDLE_VALUE) {
        const char marker[] = "\r\n--- new launch ---\r\n";
        DWORD written;
        WriteFile(log_file, marker, sizeof(marker) - 1, &written, NULL);
        CloseHandle(log_file);
    }
    copy(cmd, 4096, L"\""); append(cmd, 4096, launcher); append(cmd, 4096, L"\"");
    STARTUPINFOW si; PROCESS_INFORMATION pi;
    unsigned char *p = (unsigned char *)&si; for (SIZE_T i = 0; i < sizeof(si); i++) p[i] = 0;
    p = (unsigned char *)&pi; for (SIZE_T i = 0; i < sizeof(pi); i++) p[i] = 0;
    si.cb = sizeof(si);
    say(L"正在启动原启动器，并等待游戏启动……\r\n");
#ifdef ULTIMATE_TEST
    const DWORD flags = 0;
#else
    const DWORD flags = CREATE_NEW_CONSOLE;
#endif
    if (!CreateProcessW(launcher, cmd, NULL, NULL, FALSE, flags, NULL, launcher_dir, &si, &pi))
        finish_error(L"无法启动原启动器。请以管理员身份运行本程序并检查路径。 ");
    CloseHandle(pi.hThread);
    DWORD game = 0;
    for (int i = 0; i < 18000; i++) {
        game = find_game(game_path);
        if (game) break;
        if (WaitForSingleObject(pi.hProcess, 0) == WAIT_OBJECT_0) break;
        Sleep(10);
    }
    CloseHandle(pi.hProcess);
    if (!game) finish_error(L"没有检测到游戏启动；请检查原启动器日志或游戏路径。 ");
    if (!inject(game, dll)) finish_error(L"未能加载大招动画功能；游戏和原启动器仍可独立运行。 ");
    say(L"加载请求已发送。打开游戏设置 > 画面，查看“至终的仪式动画”。\r\n");
    say(L"如选项未出现，请查看本目录的 ultimate-skip.log。按回车键关闭此窗口。\r\n");
    WCHAR input[4]; DWORD read;
    ReadConsoleW(GetStdHandle(STD_INPUT_HANDLE), input, 4, &read, NULL);
    ExitProcess(0);
}
