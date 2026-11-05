#define WIN32_LEAN_AND_MEAN
#include <windows.h>
void mainCRTStartup(void) {
    Sleep(1500);
    if (!LoadLibraryA("tolua.dll")) ExitProcess(1);
    Sleep(700);
    const char *message = "incompatible runtime left running\r\n";
    DWORD length = 0, written;
    while (message[length]) length++;
    WriteFile(GetStdHandle(STD_OUTPUT_HANDLE), message, length, &written, NULL);
    ExitProcess(0);
}
