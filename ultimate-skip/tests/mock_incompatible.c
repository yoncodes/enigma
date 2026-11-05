#define WIN32_LEAN_AND_MEAN
#include <windows.h>
__declspec(dllexport) int luaL_loadbuffer(void *state, const char *data, SIZE_T size, const char *name) {
    return 0;
}
BOOL WINAPI DllMain(HINSTANCE module, DWORD reason, LPVOID reserved) { return TRUE; }
