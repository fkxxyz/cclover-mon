#define UNICODE
#define _UNICODE
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <shellapi.h>
#include <stdint.h>
#include <stddef.h>
#include "native_scene.h"
#include "windows/message_policy.h"
#include "windows_geometry.h"

#define CCLOVER_WM_TRAY (WM_APP + 7)
#define CCLOVER_WM_STATE (WM_APP + 8)
#define CCLOVER_MENU_QUIT 1001
#define CCLOVER_MARGIN 16
#ifndef WM_DPICHANGED
#define WM_DPICHANGED 0x02E0
#endif

typedef BOOL (WINAPI *CcloverSetProcessDpiAwarenessContextFn)(HANDLE);
typedef HRESULT (WINAPI *CcloverSetProcessDpiAwarenessFn)(int);
typedef BOOL (WINAPI *CcloverSetProcessDPIAwareFn)(void);
typedef UINT (WINAPI *CcloverGetDpiForWindowFn)(HWND);
typedef UINT (WINAPI *CcloverGetDpiForSystemFn)(void);
typedef HRESULT (WINAPI *CcloverGetDpiForMonitorFn)(HMONITOR, int, UINT *, UINT *);

typedef struct {
    HMODULE shcore;
    CcloverSetProcessDpiAwarenessContextFn set_process_context;
    CcloverSetProcessDpiAwarenessFn set_process_awareness;
    CcloverSetProcessDPIAwareFn set_process_aware;
    CcloverGetDpiForWindowFn get_window_dpi;
    CcloverGetDpiForSystemFn get_system_dpi;
    CcloverGetDpiForMonitorFn get_monitor_dpi;
} CcloverDpiApi;

typedef struct {
    UINT dpi;
    HMONITOR monitor;
    RECT work_area;
} CcloverDisplayState;

typedef struct {
    void *context;
    const CcloverCallbacks *callbacks;
    NOTIFYICONDATAW tray;
    HFONT fonts[32][2];
    UINT taskbar_created;
    CcloverDpiApi dpi_api;
    CcloverDisplayState display;
} CcloverHost;

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

#include "windows/display.inc"
#include "windows/drawing.inc"
#include "windows/shell.inc"
