#ifndef CCLOVER_WINDOWS_DISPLAY_H
#define CCLOVER_WINDOWS_DISPLAY_H

#define WIN32_LEAN_AND_MEAN
#include <windows.h>

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
    UINT dpi;
    HMONITOR monitor;
    RECT work_area;
} CcloverDisplay;

void cclover_display_init(CcloverDisplay *display);
void cclover_display_enable_dpi_awareness(const CcloverDisplay *display);
void cclover_display_refresh(CcloverDisplay *display, HWND hwnd);
void cclover_display_set(CcloverDisplay *display, HMONITOR monitor, UINT dpi);
void cclover_display_destroy(CcloverDisplay *display);
UINT cclover_display_dpi(const CcloverDisplay *display);
RECT cclover_display_work_area(const CcloverDisplay *display);
int cclover_display_px(const CcloverDisplay *display, float logical);

#endif
