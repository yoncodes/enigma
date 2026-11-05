#define WIN32_LEAN_AND_MEAN
#include <windows.h>
void mainCRTStartup(void) {
    STARTUPINFOW si; PROCESS_INFORMATION pi;
    unsigned char *p = (unsigned char *)&si; for (SIZE_T i = 0; i < sizeof(si); i++) p[i] = 0;
    p = (unsigned char *)&pi; for (SIZE_T i = 0; i < sizeof(pi); i++) p[i] = 0;
    si.cb = sizeof(si);
    SECURITY_ATTRIBUTES sa = { sizeof(sa), NULL, TRUE };
    HANDLE output = CreateFileW(L"mock-game.log", GENERIC_WRITE, FILE_SHARE_READ | FILE_SHARE_WRITE,
                                &sa, CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, NULL);
    if (output == INVALID_HANDLE_VALUE) ExitProcess(1);
    si.dwFlags = STARTF_USESTDHANDLES;
    si.hStdOutput = output; si.hStdError = output; si.hStdInput = GetStdHandle(STD_INPUT_HANDLE);
    WCHAR command[] = L"reverse1999.exe";
    if (!CreateProcessW(NULL, command, NULL, NULL, TRUE, 0, NULL, NULL, &si, &pi)) ExitProcess(1);
    CloseHandle(output);
    CloseHandle(pi.hThread);
    WaitForSingleObject(pi.hProcess, 15000);
    DWORD code = 1;
    GetExitCodeProcess(pi.hProcess, &code);
    CloseHandle(pi.hProcess);
    ExitProcess(code);
}
