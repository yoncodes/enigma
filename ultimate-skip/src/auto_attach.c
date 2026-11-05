/* In-process automatic installer for the version-locked experimental hook. */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <tlhelp32.h>

int __cdecl ultimate_hook_prepare(void);
int __cdecl ultimate_hook_commit(void);
void *__cdecl ultimate_hook_target(void);

__declspec(dllexport) void __cdecl ultimate_hook_log(const char *message) {
    static WCHAR path[2048];
    DWORD n = GetEnvironmentVariableW(L"R1999_ULTIMATE_SKIP_LOG", path, 2048);
    if (!n || n >= 2048) return;
    HANDLE file = CreateFileW(path, FILE_APPEND_DATA, FILE_SHARE_READ | FILE_SHARE_WRITE,
                              NULL, OPEN_ALWAYS, FILE_ATTRIBUTE_NORMAL, NULL);
    if (file == INVALID_HANDLE_VALUE) return;
    DWORD count = 0, written;
    while (message[count]) count++;
    WriteFile(file, message, count, &written, NULL);
    CloseHandle(file);
}

typedef struct { DWORD id; HANDLE handle; } Stopped;

static void resume_all(Stopped *stopped, DWORD count) {
    while (count) {
        count--;
        ResumeThread(stopped[count].handle);
        CloseHandle(stopped[count].handle);
    }
}

/* Suspend the other threads and verify that none is inside the bytes to patch.
 * New threads are sought again; failure leaves the original code untouched. */
static int install_at_safe_point(void) {
    static Stopped stopped[256];
    DWORD count = 0, pid = GetCurrentProcessId(), self = GetCurrentThreadId();
    unsigned char *target = (unsigned char *)ultimate_hook_target();
    int stable = 0, ok = 0;
    for (int pass = 0; pass < 6; pass++) {
        HANDLE snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
        if (snapshot == INVALID_HANDLE_VALUE) goto done;
        THREADENTRY32 entry;
        entry.dwSize = sizeof(entry);
        BOOL more = Thread32First(snapshot, &entry);
        if (!more) { CloseHandle(snapshot); goto done; }
        DWORD added = 0;
        while (more) {
            if (entry.th32OwnerProcessID == pid && entry.th32ThreadID != self) {
                int known = 0;
                for (DWORD i = 0; i < count; i++) if (stopped[i].id == entry.th32ThreadID) known = 1;
                if (!known) {
                    if (count == 256) { CloseHandle(snapshot); goto done; }
                    HANDLE thread = OpenThread(THREAD_SUSPEND_RESUME | THREAD_GET_CONTEXT | THREAD_QUERY_INFORMATION,
                                               FALSE, entry.th32ThreadID);
                    if (!thread) {
                        if (GetLastError() != ERROR_INVALID_PARAMETER) { CloseHandle(snapshot); goto done; }
                    } else if (SuspendThread(thread) == (DWORD)-1) {
                        CloseHandle(thread); CloseHandle(snapshot); goto done;
                    } else {
                        stopped[count].id = entry.th32ThreadID;
                        stopped[count++].handle = thread;
                        added++;
                    }
                }
            }
            more = Thread32Next(snapshot, &entry);
        }
        CloseHandle(snapshot);
        if (added == 0) { stable = 1; break; }
    }
    if (!stable) goto done;
    for (DWORD i = 0; i < count; i++) {
        static CONTEXT context;
        unsigned char *bytes = (unsigned char *)&context;
        for (SIZE_T j = 0; j < sizeof(context); j++) bytes[j] = 0;
        context.ContextFlags = CONTEXT_CONTROL;
        if (!GetThreadContext(stopped[i].handle, &context)) goto done;
        if (context.Rip >= (DWORD64)target && context.Rip < (DWORD64)(target + 14)) goto done;
    }
    ok = ultimate_hook_commit();
done:
    resume_all(stopped, count);
    return ok;
}

static DWORD WINAPI install_worker(void *unused) {
    ultimate_hook_log("ultimate skip loader started\r\n");
    for (int i = 0; i < 12000; i++) {
        if (GetModuleHandleA("tolua.dll")) {
            if (!ultimate_hook_prepare()) {
                ultimate_hook_log("runtime version mismatch; hook not installed\r\n");
                return 1;
            }
            for (int attempt = 0; attempt < 100; attempt++) {
                if (install_at_safe_point()) {
                    ultimate_hook_log("runtime hook installed\r\n");
                    return 0;
                }
                Sleep(10);
            }
            ultimate_hook_log("could not obtain a safe installation point\r\n");
            return 1;
        }
        Sleep(5);
    }
    ultimate_hook_log("timed out waiting for tolua.dll\r\n");
    return 1;
}

BOOL WINAPI DllMain(HINSTANCE module, DWORD reason, LPVOID reserved) {
    if (reason == DLL_PROCESS_ATTACH) {
        DisableThreadLibraryCalls(module);
        HANDLE worker = CreateThread(NULL, 0, install_worker, NULL, 0, NULL);
        if (worker) CloseHandle(worker);
    }
    return TRUE;
}
