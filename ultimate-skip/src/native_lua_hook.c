/* Version-locked experimental hook for the game's tolua_loadbuffer wrapper.
 * The LuaJIT luaL_loadbuffer entry remains untouched and is used as the
 * original implementation; no copied-prologue trampoline is needed.
 */
#include "payload.h"
typedef unsigned long DWORD;
typedef unsigned long long SIZE_T;
typedef int BOOL;
typedef void *HANDLE;
typedef void *HMODULE;
__declspec(dllimport) HMODULE __stdcall GetModuleHandleA(const char *);
__declspec(dllimport) void *__stdcall GetProcAddress(HMODULE, const char *);
__declspec(dllimport) BOOL __stdcall VirtualProtect(void *, SIZE_T, DWORD, DWORD *);
__declspec(dllimport) HANDLE __stdcall GetCurrentProcess(void);
__declspec(dllimport) BOOL __stdcall FlushInstructionCache(HANDLE, const void *, SIZE_T);

typedef int (__cdecl *load_fn)(void *, const char *, SIZE_T, const char *);
static load_fn original_load;
static unsigned char *prepared_target;
static int installed;
static unsigned int replaced;
static const unsigned char expected_prologue[14] = {
    0x4d, 0x63, 0xc0, 0xe9, 0x38, 0x18, 0x03, 0x00,
    0x0f, 0x1f, 0x84, 0x00, 0x00, 0x00
};

static int equal_bytes(const unsigned char *a, const unsigned char *b, SIZE_T n) {
    for (SIZE_T i = 0; i < n; i++) if (a[i] != b[i]) return 0;
    return 1;
}
static int original_name(const char *name) {
    static const char expected[] = "@ultimate_skip.original";
    if (!name) return 0;
    for (SIZE_T i = 0; i < sizeof(expected); i++) if (name[i] != expected[i]) return 0;
    return 1;
}
static void write_jump(unsigned char *target, void *destination) {
    SIZE_T address = (SIZE_T)destination;
    target[0] = 0xff; target[1] = 0x25;
    target[2] = 0; target[3] = 0; target[4] = 0; target[5] = 0;
    for (int i = 0; i < 8; i++) target[6 + i] = (unsigned char)(address >> (i * 8));
}

static int __cdecl replacement_load(void *state, const char *buffer, int length, const char *name) {
    SIZE_T size = (SIZE_T)(long long)length;
    if (length >= 0 && buffer && size == sizeof(original_module) && !original_name(name)
        && equal_bytes((const unsigned char *)buffer, original_module, size)) {
        replaced++;
#ifdef ULTIMATE_AUTO_INSTALL
        extern void ultimate_hook_log(const char *);
        ultimate_hook_log("battle module replaced\r\n");
#endif
        return original_load(state, (const char *)candidate_module, sizeof(candidate_module), name);
    }
    if (length >= 0 && buffer && size == sizeof(original_settings) && !original_name(name)
        && equal_bytes((const unsigned char *)buffer, original_settings, size)) {
        replaced++;
#ifdef ULTIMATE_AUTO_INSTALL
        extern void ultimate_hook_log(const char *);
        ultimate_hook_log("settings module replaced\r\n");
#endif
        return original_load(state, (const char *)candidate_settings, sizeof(candidate_settings), name);
    }
    return original_load(state, buffer, size, name);
}

__declspec(dllexport) unsigned int __cdecl ultimate_hook_hits(void) { return replaced; }

__declspec(dllexport) int __cdecl ultimate_hook_prepare(void) {
    if (installed) return 1;
    if (prepared_target) return 1;
    HMODULE module = GetModuleHandleA("tolua.dll");
    if (!module) return 0;
    unsigned char *target = (unsigned char *)GetProcAddress(module, "tolua_loadbuffer");
    original_load = (load_fn)GetProcAddress(module, "luaL_loadbuffer");
    if (!target || !original_load || !equal_bytes(target, expected_prologue, sizeof(expected_prologue))) return 0;
    prepared_target = target;
    return 1;
}

__declspec(dllexport) void *__cdecl ultimate_hook_target(void) { return prepared_target; }

__declspec(dllexport) int __cdecl ultimate_hook_commit(void) {
    if (installed) return 1;
    unsigned char *target = prepared_target;
    if (!target || !original_load || !equal_bytes(target, expected_prologue, sizeof(expected_prologue))) return 0;
    DWORD old_protection = 0, ignored = 0;
    if (!VirtualProtect(target, 14, 0x40, &old_protection)) return 0;
    write_jump(target, (void *)replacement_load);
    VirtualProtect(target, 14, old_protection, &ignored);
    FlushInstructionCache(GetCurrentProcess(), target, 14);
    installed = 1;
    return 1;
}

__declspec(dllexport) int __cdecl ultimate_hook_install(void) {
    return ultimate_hook_prepare() && ultimate_hook_commit();
}

#ifndef ULTIMATE_AUTO_INSTALL
BOOL __stdcall DllMain(void *module, DWORD reason, void *reserved) { return 1; }
#endif
