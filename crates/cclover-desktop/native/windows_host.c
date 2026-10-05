#define UNICODE
#define _UNICODE
#define WIN32_LEAN_AND_MEAN
#include <windows.h>

int cclover_win32_desktop_available(void) {
    HWINSTA station = GetProcessWindowStation();
    USEROBJECTFLAGS flags;
    DWORD needed = 0;
    if (station == NULL ||
        !GetUserObjectInformationW(station, UOI_FLAGS, &flags, sizeof(flags), &needed)) {
        return 0;
    }
    return (flags.dwFlags & WSF_VISIBLE) != 0;
}
