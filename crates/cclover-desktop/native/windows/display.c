#define UNICODE
#define _UNICODE
#include "display.h"
#include "windows_geometry.h"

static UINT cclover_system_dpi(const CcloverDisplay *display) {
    HDC screen;
    int dpi;
    if (display->get_system_dpi) {
        UINT current = display->get_system_dpi();
        if (current) return current;
    }
    screen = GetDC(NULL);
    if (!screen) return CCLOVER_DEFAULT_DPI;
    dpi = GetDeviceCaps(screen, LOGPIXELSX);
    ReleaseDC(NULL, screen);
    return dpi > 0 ? (UINT)dpi : CCLOVER_DEFAULT_DPI;
}

static UINT cclover_monitor_dpi(const CcloverDisplay *display, HMONITOR monitor) {
    UINT x = 0, y = 0;
    if (monitor && display->get_monitor_dpi &&
        SUCCEEDED(display->get_monitor_dpi(monitor, 0, &x, &y)) && x) {
        return x;
    }
    return cclover_system_dpi(display);
}

static UINT cclover_window_dpi(const CcloverDisplay *display, HWND hwnd, HMONITOR monitor) {
    if (hwnd && display->get_window_dpi) {
        UINT dpi = display->get_window_dpi(hwnd);
        if (dpi) return dpi;
    }
    return cclover_monitor_dpi(display, monitor);
}

void cclover_display_init(CcloverDisplay *display) {
    HMODULE user32 = GetModuleHandleW(L"user32.dll");
    ZeroMemory(display, sizeof(*display));
    if (user32) {
        display->set_process_context = (CcloverSetProcessDpiAwarenessContextFn)
            GetProcAddress(user32, "SetProcessDpiAwarenessContext");
        display->set_process_aware = (CcloverSetProcessDPIAwareFn)
            GetProcAddress(user32, "SetProcessDPIAware");
        display->get_window_dpi = (CcloverGetDpiForWindowFn)
            GetProcAddress(user32, "GetDpiForWindow");
        display->get_system_dpi = (CcloverGetDpiForSystemFn)
            GetProcAddress(user32, "GetDpiForSystem");
    }
    display->shcore = LoadLibraryW(L"shcore.dll");
    if (display->shcore) {
        display->set_process_awareness = (CcloverSetProcessDpiAwarenessFn)
            GetProcAddress(display->shcore, "SetProcessDpiAwareness");
        display->get_monitor_dpi = (CcloverGetDpiForMonitorFn)
            GetProcAddress(display->shcore, "GetDpiForMonitor");
    }
}

void cclover_display_enable_dpi_awareness(const CcloverDisplay *display) {
    if (display->set_process_context &&
        display->set_process_context((HANDLE)(INT_PTR)-4)) {
        return;
    }
    if (display->set_process_awareness &&
        SUCCEEDED(display->set_process_awareness(2))) return;
    if (display->set_process_aware) display->set_process_aware();
}

void cclover_display_set(CcloverDisplay *display, HMONITOR monitor, UINT dpi) {
    MONITORINFO info;
    if (!monitor) {
        POINT origin = {0, 0};
        monitor = MonitorFromPoint(origin, MONITOR_DEFAULTTOPRIMARY);
    }
    if (!dpi) dpi = cclover_monitor_dpi(display, monitor);
    if (!dpi) dpi = CCLOVER_DEFAULT_DPI;
    display->dpi = dpi;
    display->monitor = monitor;
    ZeroMemory(&info, sizeof(info));
    info.cbSize = sizeof(info);
    if (monitor && GetMonitorInfoW(monitor, &info)) {
        display->work_area = info.rcWork;
    } else {
        display->work_area.left = 0;
        display->work_area.top = 0;
        display->work_area.right = GetSystemMetrics(SM_CXSCREEN);
        display->work_area.bottom = GetSystemMetrics(SM_CYSCREEN);
    }
}

void cclover_display_refresh(CcloverDisplay *display, HWND hwnd) {
    HMONITOR monitor = hwnd
        ? MonitorFromWindow(hwnd, MONITOR_DEFAULTTOPRIMARY)
        : NULL;
    UINT dpi = cclover_window_dpi(display, hwnd, monitor);
    cclover_display_set(display, monitor, dpi);
}

void cclover_display_destroy(CcloverDisplay *display) {
    if (display->shcore) {
        FreeLibrary(display->shcore);
        display->shcore = NULL;
    }
}

UINT cclover_display_dpi(const CcloverDisplay *display) {
    return display->dpi;
}

RECT cclover_display_work_area(const CcloverDisplay *display) {
    return display->work_area;
}

int cclover_display_px(const CcloverDisplay *display, float logical) {
    return cclover_scale_logical(logical, display->dpi);
}
