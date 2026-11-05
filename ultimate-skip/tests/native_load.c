/* Loads the original and candidate LuaJIT chunks with the game's own tolua.dll.
 * The test does not start the game or modify any game files. */
typedef unsigned long DWORD;
typedef int BOOL;
typedef void *HANDLE;
typedef void *HMODULE;
typedef const char *LPCSTR;
typedef unsigned long long SIZE_T;

__declspec(dllimport) HMODULE __stdcall LoadLibraryA(LPCSTR);
__declspec(dllimport) HMODULE __stdcall GetModuleHandleA(LPCSTR);
__declspec(dllimport) void __stdcall Sleep(DWORD);
__declspec(dllimport) void *__stdcall GetProcAddress(HMODULE, LPCSTR);
__declspec(dllimport) HANDLE __stdcall CreateFileA(LPCSTR, DWORD, DWORD, void *, DWORD, DWORD, HANDLE);
__declspec(dllimport) DWORD __stdcall GetFileSize(HANDLE, DWORD *);
__declspec(dllimport) BOOL __stdcall ReadFile(HANDLE, void *, DWORD, DWORD *, void *);
__declspec(dllimport) BOOL __stdcall CloseHandle(HANDLE);
__declspec(dllimport) HANDLE __stdcall GetStdHandle(DWORD);
__declspec(dllimport) BOOL __stdcall WriteFile(HANDLE, const void *, DWORD, DWORD *, void *);
__declspec(dllimport) HANDLE __stdcall GetProcessHeap(void);
__declspec(dllimport) void *__stdcall HeapAlloc(HANDLE, DWORD, SIZE_T);
__declspec(dllimport) void __stdcall ExitProcess(DWORD);

typedef void *(__cdecl *newstate_fn)(void);
typedef void (__cdecl *openlibs_fn)(void *);
typedef int (__cdecl *load_fn)(void *, const char *, SIZE_T, const char *);
typedef int (__cdecl *pcall_fn)(void *, int, int, int);
typedef const char *(__cdecl *error_fn)(void *, int, SIZE_T *);
typedef void (__cdecl *close_fn)(void *);
typedef void (__cdecl *setfield_fn)(void *, int, const char *);

static SIZE_T length(const char *s) { SIZE_T n = 0; while (s[n]) n++; return n; }
static void print(const char *s) {
    DWORD written;
    WriteFile(GetStdHandle((DWORD)-11), s, (DWORD)length(s), &written, 0);
}
static void fail(const char *stage, void *state, error_fn error) {
    print(stage);
    if (state && error) {
        const char *message = error(state, -1, 0);
        if (message) { print(": "); print(message); }
    }
    print("\r\n");
    ExitProcess(1);
}
static char *read_all(const char *name, DWORD *size) {
    HANDLE file = CreateFileA(name, 0x80000000u, 1, 0, 3, 0x80, 0);
    if (file == (HANDLE)-1) return 0;
    *size = GetFileSize(file, 0);
    char *data = (char *)HeapAlloc(GetProcessHeap(), 0, *size);
    DWORD actual = 0;
    if (!data || !ReadFile(file, data, *size, &actual, 0) || actual != *size) return 0;
    CloseHandle(file);
    return data;
}

void mainCRTStartup(void) {
#ifdef TEST_AUTO_INJECTION
    Sleep(1500); /* The separate launcher gets time to inject before Lua loads. */
#endif
    HMODULE dll = LoadLibraryA("tolua.dll");
    if (!dll) fail("cannot load tolua.dll", 0, 0);
#if defined(TEST_RUNTIME_HOOK) || defined(TEST_AUTO_INJECTION)
#ifdef TEST_AUTO_INJECTION
    Sleep(500);
    HMODULE hook = GetModuleHandleA("ultimate-skip-hook.dll");
#else
    HMODULE hook = LoadLibraryA("ultimate_hook.dll");
#endif
    if (!hook) fail("cannot load experimental hook", 0, 0);
    int (__cdecl *install)(void) = (int (__cdecl *)(void))GetProcAddress(hook, "ultimate_hook_install");
    unsigned int (__cdecl *hits)(void) = (unsigned int (__cdecl *)(void))GetProcAddress(hook, "ultimate_hook_hits");
#ifdef TEST_RUNTIME_HOOK
    if (!install || !hits || !install()) fail("hook did not match this runtime", 0, 0);
#else
    if (!install || !hits) fail("missing automatic hook exports", 0, 0);
#endif
#endif
    newstate_fn newstate = (newstate_fn)GetProcAddress(dll, "luaL_newstate");
    openlibs_fn openlibs = (openlibs_fn)GetProcAddress(dll, "luaL_openlibs");
    load_fn load = (load_fn)GetProcAddress(dll, "luaL_loadbuffer");
    load_fn game_load = (load_fn)GetProcAddress(dll, "tolua_loadbuffer");
    pcall_fn pcall = (pcall_fn)GetProcAddress(dll, "lua_pcall");
    error_fn error = (error_fn)GetProcAddress(dll, "lua_tolstring");
    close_fn close = (close_fn)GetProcAddress(dll, "lua_close");
    setfield_fn setfield = (setfield_fn)GetProcAddress(dll, "lua_setfield");
    if (!newstate || !openlibs || !load || !game_load || !pcall || !error || !close || !setfield) fail("missing Lua exports", 0, 0);
#if defined(TEST_RUNTIME_HOOK) || defined(TEST_AUTO_INJECTION)
    static const unsigned char expected_lua_prologue[14] = {
        0x48,0x83,0xec,0x48,0x48,0x89,0x54,0x24,0x30,0x4c,0x89,0x44,0x24,0x38
    };
    for (int i = 0; i < 14; i++)
        if (((const unsigned char *)load)[i] != expected_lua_prologue[i])
            fail("luaL_loadbuffer was modified", 0, 0);
#endif

    void *state = newstate();
    if (!state) fail("cannot create Lua state", 0, 0);
    openlibs(state);
    const char *setup = "FightBaseClass={} FightWorkItem={} BaseView={} function class(n,b) local c={} c.__index=c "
        "setmetatable(c,{__index=b}) return c end";
    if (load(state, setup, length(setup), "setup") || pcall(state, 0, 0, 0)) fail("setup", state, error);

    DWORD size = 0;
    char *skill = read_all("timeline_original.luao", &size);
    if (!skill || game_load(state, skill, size, "FightWorkTimelineItem") || pcall(state, 0, 1, 0))
        fail("original timeline component", state, error);
    setfield(state, -10002, "FightWorkTimelineItem");
    const char *check_skill = "assert(type(FightWorkTimelineItem.onConstructor)=='function' and "
        "FightWorkTimelineItem._ultimateSkipPrototypeInstalled==nil)";
    if (load(state, check_skill, length(check_skill), "check_skill") || pcall(state, 0, 0, 0))
        fail("entity component verification", state, error);
#if defined(TEST_RUNTIME_HOOK) || defined(TEST_AUTO_INJECTION)
    if (hits() != 0) fail("supplementary timeline module was unexpectedly intercepted", 0, 0);
#endif

    char *original = read_all("original.luao", &size);
    if (!original) fail("cannot read original chunk", 0, 0);
#if defined(TEST_RUNTIME_HOOK) || defined(TEST_AUTO_INJECTION)
    if (load(state, original, size, "direct LuaJIT") || pcall(state, 0, 0, 0))
        fail("unmodified direct LuaJIT path", state, error);
    if (hits() != 0) fail("direct LuaJIT path was unexpectedly intercepted", 0, 0);
    if (game_load(state, original, size, "original") || pcall(state, 0, 0, 0)) fail("hooked original bytecode", state, error);
    if (hits() != 1) fail("original module was not replaced exactly once", 0, 0);
#ifdef TEST_AUTO_INJECTION
    const char *check_mode = "PlayerPrefsHelper={getNumber=function(_,fallback) return fallback end} "
        "assert(UltimateSkipPrototype.getMode()==2)";
    if (load(state, check_mode, length(check_mode), "check_mode") || pcall(state, 0, 0, 0))
        fail("default mode was not inherited", state, error);
    const char *check_debug = "local ffi=require('ffi'); ffi.cdef('void ultimate_hook_log(const char *);'); "
        "ffi.load('ultimate-skip-hook.dll').ultimate_hook_log('logging bridge verified\\r\\n')";
    if (load(state, check_debug, length(check_debug), "check_debug") || pcall(state, 0, 0, 0))
        fail("logging bridge", state, error);
#endif
    print("original module intercepted exactly once\r\n");
#else
    if (load(state, original, size, "original")) fail("original bytecode", state, error);
    print("original bytecode accepted\r\n");

    char *candidate = read_all("candidate.luao", &size);
    if (!candidate) fail("cannot read candidate chunk", 0, 0);
    if (load(state, candidate, size, "candidate") || pcall(state, 0, 0, 0)) fail("candidate module", state, error);
    const char *verify = "assert(type(UltimateSkipPrototype)=='table' and UltimateSkipPrototype.enabled==false)";
    if (load(state, verify, length(verify), "verify") || pcall(state, 0, 0, 0)) fail("candidate verification", state, error);
    print("candidate module accepted and installed (disabled)\r\n");
#endif
    char *settings = read_all(
#if defined(TEST_RUNTIME_HOOK) || defined(TEST_AUTO_INJECTION)
        "settings_original.luao",
#else
        "settings_candidate.luao",
#endif
        &size);
    if (!settings || game_load(state, settings, size, "SettingsGraphicsView") || pcall(state, 0, 1, 0))
        fail("settings module", state, error);
    setfield(state, -10002, "SettingsGraphicsView");
    const char *check_settings = "assert(SettingsGraphicsView._ultimateSkipSettingsInstalled)";
    if (load(state, check_settings, length(check_settings), "check_settings") || pcall(state, 0, 0, 0))
        fail("settings verification", state, error);
#if defined(TEST_RUNTIME_HOOK) || defined(TEST_AUTO_INJECTION)
    if (hits() != 2) fail("settings module was not replaced", 0, 0);
#endif
    close(state);

    state = newstate();
    if (!state) fail("cannot create integration Lua state", 0, 0);
    openlibs(state);
    char *integration = read_all("integration_setup.lua", &size);
    if (!integration) fail("cannot read integration setup", 0, 0);
    if (load(state, integration, size, "integration_setup") || pcall(state, 0, 0, 0))
        fail("integration setup", state, error);
#if defined(TEST_RUNTIME_HOOK) || defined(TEST_AUTO_INJECTION)
    char *module = read_all("original.luao", &size);
#else
    char *module = read_all("candidate.luao", &size);
#endif
    if (!module || game_load(state, module, size, "FightSkillComp") || pcall(state, 0, 1, 0))
        fail("integration module", state, error);
    setfield(state, -10002, "FightSkillComp"); /* LUA_GLOBALSINDEX, LuaJIT 2.1 */
    integration = read_all("integration_checks.lua", &size);
    if (!integration) fail("cannot read integration checks", 0, 0);
    if (load(state, integration, size, "integration_checks") || pcall(state, 0, 0, 0))
        fail("integration tests", state, error);
#if defined(TEST_RUNTIME_HOOK) || defined(TEST_AUTO_INJECTION)
    if (hits() != 3) fail("integration did not use the runtime replacement", 0, 0);
#endif
    close(state);

    /* The real game's global guard rejected the old export and interrupted
     * require(). Run that original boot code in the game's own Lua runtime. */
    state = newstate();
    if (!state) fail("cannot create bootstrap regression state", 0, 0);
    openlibs(state);
    char *bootstrap_setup = read_all("bootstrap_setup.lua", &size);
    if (!bootstrap_setup || load(state, bootstrap_setup, size, "bootstrap_setup") || pcall(state, 0, 0, 0))
        fail("bootstrap prerequisites", state, error);
    char *boot = read_all("boot_original.luao", &size);
    if (!boot || load(state, boot, size, "BootStarter") || pcall(state, 0, 0, 0))
        fail("original BootStarter", state, error);
    const char *restore_require = "require=savedBootstrapRequire; assert(#bootstrapErrors==0)";
    if (load(state, restore_require, length(restore_require), "restore_require") || pcall(state, 0, 0, 0))
        fail("bootstrap completed without errors", state, error);
    char *oop = read_all("oop_original.luao", &size);
    if (!oop || load(state, oop, size, "oop") || pcall(state, 0, 0, 0))
        fail("original class implementation", state, error);
    char *guarded_module = read_all(
#if defined(TEST_RUNTIME_HOOK) || defined(TEST_AUTO_INJECTION)
        "original.luao",
#else
        "candidate.luao",
#endif
        &size);
    if (!guarded_module || game_load(state, guarded_module, size, "FightSkillComp guarded") || pcall(state, 0, 0, 0))
        fail("module load with game global guard", state, error);
    const char *check_guard = "assert(type(rawget(_G,'UltimateSkipPrototype'))=='table'); assert(#bootstrapErrors==0)";
    if (load(state, check_guard, length(check_guard), "check_guard") || pcall(state, 0, 0, 0))
        fail("global registration regression", state, error);
    print("game global guard regression passed\r\n");
#if defined(TEST_RUNTIME_HOOK) || defined(TEST_AUTO_INJECTION)
    if (hits() != 4) fail("bootstrap regression did not use the runtime replacement", 0, 0);
    print("runtime interception and eleven effect/camera tests passed with unmodified assets\r\n");
#else
    print("eleven effect/camera and fallback tests passed with game tolua.dll\r\n");
#endif
    close(state);
    ExitProcess(0);
}
