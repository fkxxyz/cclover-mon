#define UNICODE
#define _UNICODE
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <shellapi.h>
#include <stdint.h>
#include <stddef.h>
#include "native_scene.h"
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
    HDC measure_dc;
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

static COLORREF cclover_color(uint32_t argb) {
    unsigned a = (argb >> 24) & 0xff;
    unsigned r = (argb >> 16) & 0xff;
    unsigned g = (argb >> 8) & 0xff;
    unsigned b = argb & 0xff;
    if (a < 255) {
        const unsigned br = 0x0b, bg = 0x10, bb = 0x20;
        r = (r * a + br * (255 - a)) / 255;
        g = (g * a + bg * (255 - a)) / 255;
        b = (b * a + bb * (255 - a)) / 255;
    }
    return RGB(r, g, b);
}

static int cclover_px(const CcloverHost *host, float logical) {
    return cclover_scale_logical(logical, host->display.dpi);
}

static float cclover_logical_px(const CcloverHost *host, int physical) {
    return cclover_unscale_physical(physical, host->display.dpi);
}

static void cclover_init_dpi_api(CcloverDpiApi *api) {
    HMODULE user32 = GetModuleHandleW(L"user32.dll");
    ZeroMemory(api, sizeof(*api));
    if (user32) {
        api->set_process_context = (CcloverSetProcessDpiAwarenessContextFn)
            GetProcAddress(user32, "SetProcessDpiAwarenessContext");
        api->set_process_aware = (CcloverSetProcessDPIAwareFn)
            GetProcAddress(user32, "SetProcessDPIAware");
        api->get_window_dpi = (CcloverGetDpiForWindowFn)
            GetProcAddress(user32, "GetDpiForWindow");
        api->get_system_dpi = (CcloverGetDpiForSystemFn)
            GetProcAddress(user32, "GetDpiForSystem");
    }
    api->shcore = LoadLibraryW(L"shcore.dll");
    if (api->shcore) {
        api->set_process_awareness = (CcloverSetProcessDpiAwarenessFn)
            GetProcAddress(api->shcore, "SetProcessDpiAwareness");
        api->get_monitor_dpi = (CcloverGetDpiForMonitorFn)
            GetProcAddress(api->shcore, "GetDpiForMonitor");
    }
}

static void cclover_enable_dpi_awareness(const CcloverDpiApi *api) {
    if (api->set_process_context &&
        api->set_process_context((HANDLE)(INT_PTR)-4)) {
        return;
    }
    if (api->set_process_awareness && SUCCEEDED(api->set_process_awareness(2))) return;
    if (api->set_process_aware) api->set_process_aware();
}

static UINT cclover_system_dpi(const CcloverDpiApi *api) {
    HDC screen;
    int dpi;
    if (api->get_system_dpi) {
        UINT current = api->get_system_dpi();
        if (current) return current;
    }
    screen = GetDC(NULL);
    if (!screen) return CCLOVER_DEFAULT_DPI;
    dpi = GetDeviceCaps(screen, LOGPIXELSX);
    ReleaseDC(NULL, screen);
    return dpi > 0 ? (UINT)dpi : CCLOVER_DEFAULT_DPI;
}

static UINT cclover_monitor_dpi(const CcloverDpiApi *api, HMONITOR monitor) {
    UINT x = 0, y = 0;
    if (monitor && api->get_monitor_dpi &&
        SUCCEEDED(api->get_monitor_dpi(monitor, 0, &x, &y)) && x) {
        return x;
    }
    return cclover_system_dpi(api);
}

static UINT cclover_window_dpi(const CcloverHost *host, HWND hwnd, HMONITOR monitor) {
    if (hwnd && host->dpi_api.get_window_dpi) {
        UINT dpi = host->dpi_api.get_window_dpi(hwnd);
        if (dpi) return dpi;
    }
    return cclover_monitor_dpi(&host->dpi_api, monitor);
}

static void cclover_release_fonts(CcloverHost *host) {
    unsigned i, j;
    for (i = 0; i < 32; ++i) {
        for (j = 0; j < 2; ++j) {
            if (host->fonts[i][j]) {
                DeleteObject(host->fonts[i][j]);
                host->fonts[i][j] = NULL;
            }
        }
    }
}

static void cclover_set_display(CcloverHost *host, HMONITOR monitor, UINT dpi) {
    MONITORINFO info;
    if (!monitor) {
        POINT origin = {0, 0};
        monitor = MonitorFromPoint(origin, MONITOR_DEFAULTTOPRIMARY);
    }
    if (!dpi) dpi = cclover_monitor_dpi(&host->dpi_api, monitor);
    if (!dpi) dpi = CCLOVER_DEFAULT_DPI;
    if (host->display.dpi &&
        cclover_dpi_requires_resource_refresh(host->display.dpi, dpi)) {
        cclover_release_fonts(host);
    }
    host->display.dpi = dpi;
    host->display.monitor = monitor;
    ZeroMemory(&info, sizeof(info));
    info.cbSize = sizeof(info);
    if (monitor && GetMonitorInfoW(monitor, &info)) {
        host->display.work_area = info.rcWork;
    } else {
        host->display.work_area.left = 0;
        host->display.work_area.top = 0;
        host->display.work_area.right = GetSystemMetrics(SM_CXSCREEN);
        host->display.work_area.bottom = GetSystemMetrics(SM_CYSCREEN);
    }
}

static void cclover_refresh_display(CcloverHost *host, HWND hwnd) {
    HMONITOR monitor = hwnd
        ? MonitorFromWindow(hwnd, MONITOR_DEFAULTTOPRIMARY)
        : NULL;
    UINT dpi = cclover_window_dpi(host, hwnd, monitor);
    cclover_set_display(host, monitor, dpi);
}

static HFONT cclover_font(CcloverHost *host, uint32_t size, int bold) {
    uint32_t slot = size < 32 ? size : 31;
    HFONT cached = host->fonts[slot][bold ? 1 : 0];
    if (cached) return cached;
    int pixels = -cclover_px(host, (float)size);
    if (pixels == 0) pixels = -(int)size;
    cached = CreateFontW(pixels, 0, 0, 0, bold ? FW_BOLD : FW_NORMAL,
        FALSE, FALSE, FALSE, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS,
        CLEARTYPE_QUALITY, FIXED_PITCH | FF_MODERN, L"Inconsolata");
    host->fonts[slot][bold ? 1 : 0] = cached;
    return cached;
}

static float cclover_measure_text(void *context, const uint8_t *text, size_t text_len,
                                  uint32_t size, uint32_t flags) {
    CcloverHost *host = context;
    wchar_t stack_text[256];
    wchar_t *wide = stack_text;
    SIZE extent = {0, 0};
    int wide_len = MultiByteToWideChar(CP_UTF8, 0, (LPCCH)text, (int)text_len, NULL, 0);
    HFONT font;
    HGDIOBJ old;
    if (wide_len <= 0) return 0.0f;
    if (wide_len > (int)(sizeof(stack_text) / sizeof(stack_text[0]))) {
        wide = (wchar_t *)HeapAlloc(GetProcessHeap(), 0,
                                    (size_t)wide_len * sizeof(wchar_t));
        if (!wide) return 0.0f;
    }
    MultiByteToWideChar(CP_UTF8, 0, (LPCCH)text, (int)text_len, wide, wide_len);
    font = cclover_font(host, size,
                        (flags & CCLOVER_TEXT_BOLD) != 0);
    old = SelectObject(host->measure_dc, font);
    GetTextExtentPoint32W(host->measure_dc, wide, wide_len, &extent);
    SelectObject(host->measure_dc, old);
    if (wide != stack_text) HeapFree(GetProcessHeap(), 0, wide);
    return cclover_logical_px(host, extent.cx);
}

static void cclover_release_resources(CcloverHost *host) {
    cclover_release_fonts(host);
    if (host->measure_dc) {
        DeleteDC(host->measure_dc);
        host->measure_dc = NULL;
    }
    if (host->dpi_api.shcore) {
        FreeLibrary(host->dpi_api.shcore);
        host->dpi_api.shcore = NULL;
    }
}

static void cclover_draw_rect(CcloverHost *host, HDC dc, const CcloverCommand *cmd, int fill) {
    int left = cclover_px(host, cmd->x), top = cclover_px(host, cmd->y);
    int right = cclover_px(host, cmd->x + cmd->width);
    int bottom = cclover_px(host, cmd->y + cmd->height);
    int diameter = cclover_px(host, cmd->radius * 2.0f);
    HGDIOBJ old_brush, old_pen;
    HBRUSH brush = fill ? CreateSolidBrush(cclover_color(cmd->color)) : (HBRUSH)GetStockObject(NULL_BRUSH);
    HPEN pen = fill ? (HPEN)GetStockObject(NULL_PEN)
                    : CreatePen(PS_SOLID, max(1, cclover_px(host, cmd->stroke_width)), cclover_color(cmd->color));
    old_brush = SelectObject(dc, brush);
    old_pen = SelectObject(dc, pen);
    if (diameter > 0) RoundRect(dc, left, top, right, bottom, diameter, diameter);
    else Rectangle(dc, left, top, right, bottom);
    SelectObject(dc, old_pen);
    SelectObject(dc, old_brush);
    if (fill) DeleteObject(brush); else DeleteObject(pen);
}

static void cclover_draw_text(CcloverHost *host, HDC dc, const CcloverCommand *cmd) {
    RECT rect;
    UINT flags = DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX;
    HFONT font = cclover_font(host, cmd->text_size,
                              (cmd->flags & CCLOVER_TEXT_BOLD) != 0);
    HGDIOBJ old = SelectObject(dc, font);
    wchar_t stack_text[256];
    wchar_t *wide = stack_text;
    int wide_len = MultiByteToWideChar(CP_UTF8, 0, (LPCCH)cmd->text,
                                       (int)cmd->text_len, NULL, 0);
    if (wide_len <= 0) {
        SelectObject(dc, old);
        return;
    }
    if (wide_len > (int)(sizeof(stack_text) / sizeof(stack_text[0]))) {
        wide = (wchar_t *)HeapAlloc(GetProcessHeap(), 0,
                                    (size_t)wide_len * sizeof(wchar_t));
        if (!wide) {
            SelectObject(dc, old);
            return;
        }
    }
    MultiByteToWideChar(CP_UTF8, 0, (LPCCH)cmd->text, (int)cmd->text_len,
                        wide, wide_len);
    rect.left = cclover_px(host, cmd->x);
    rect.top = cclover_px(host, cmd->y);
    rect.right = cclover_px(host, cmd->x + cmd->width);
    rect.bottom = cclover_px(host, cmd->y + cmd->height);
    flags |= (cmd->flags & CCLOVER_TEXT_END) ? DT_RIGHT : DT_LEFT;
    if (!(cmd->flags & CCLOVER_TEXT_CLIP)) flags |= DT_NOCLIP;
    SetTextColor(dc, cclover_color(cmd->color));
    DrawTextW(dc, wide, wide_len, &rect, flags);
    if (wide != stack_text) HeapFree(GetProcessHeap(), 0, wide);
    SelectObject(dc, old);
}

static void cclover_draw_scene(CcloverHost *host, HDC dc, const CcloverScene *scene) {
    size_t i;
    SetBkMode(dc, TRANSPARENT);
    for (i = 0; i < scene->command_count; ++i) {
        const CcloverCommand *cmd = &scene->commands[i];
        if (cmd->kind == CCLOVER_CMD_FILL_RECT) {
            cclover_draw_rect(host, dc, cmd, 1);
        } else if (cmd->kind == CCLOVER_CMD_STROKE_RECT) {
            cclover_draw_rect(host, dc, cmd, 0);
        } else if (cmd->kind == CCLOVER_CMD_TEXT) {
            cclover_draw_text(host, dc, cmd);
        } else if (cmd->kind == CCLOVER_CMD_POLYLINE ||
                   cmd->kind == CCLOVER_CMD_POLYGON) {
            size_t j;
            POINT stack_points[128];
            POINT *points = stack_points;
            if (cmd->point_count == 0 || cmd->point_offset + cmd->point_count > scene->point_count) continue;
            if (cmd->point_count > 128) {
                points = (POINT *)HeapAlloc(GetProcessHeap(), 0, cmd->point_count * sizeof(POINT));
                if (!points) continue;
            }
            for (j = 0; j < cmd->point_count; ++j) {
                const CcloverPoint *source = &scene->points[cmd->point_offset + j];
                points[j].x = cclover_px(host, source->x);
                points[j].y = cclover_px(host, source->y);
            }
            if (cmd->kind == CCLOVER_CMD_POLYLINE) {
                HPEN pen = CreatePen(PS_SOLID, max(1, cclover_px(host, cmd->stroke_width)), cclover_color(cmd->color));
                HGDIOBJ old_pen = SelectObject(dc, pen);
                Polyline(dc, points, (int)cmd->point_count);
                SelectObject(dc, old_pen);
                DeleteObject(pen);
            } else {
                HBRUSH brush = CreateSolidBrush(cclover_color(cmd->color));
                HGDIOBJ old_brush = SelectObject(dc, brush);
                HGDIOBJ old_pen = SelectObject(dc, GetStockObject(NULL_PEN));
                Polygon(dc, points, (int)cmd->point_count);
                SelectObject(dc, old_pen);
                SelectObject(dc, old_brush);
                DeleteObject(brush);
            }
            if (points != stack_points) HeapFree(GetProcessHeap(), 0, points);
        }
    }
}

static void cclover_scene(CcloverHost *host, CcloverScene *scene) {
    ZeroMemory(scene, sizeof(*scene));
    host->callbacks->scene(host->context, host, cclover_measure_text, scene);
}

static void cclover_place(CcloverHost *host, HWND hwnd, uint32_t width, uint32_t height) {
    HWND shell = GetShellWindow();
    CcloverRectI work = {
        host->display.work_area.left,
        host->display.work_area.top,
        host->display.work_area.right,
        host->display.work_area.bottom,
    };
    CcloverWindowGeometry geometry = cclover_top_right_geometry(
        width, height, work, host->display.dpi, (float)CCLOVER_MARGIN);
    int radius = cclover_px(host, 32.0f);
    if (shell && GetWindow(hwnd, GW_OWNER) != shell) {
        SetWindowLongPtrW(hwnd, GWLP_HWNDPARENT, (LONG_PTR)shell);
    }
    SetWindowPos(hwnd, HWND_BOTTOM, geometry.x, geometry.y, geometry.width, geometry.height,
                 SWP_NOACTIVATE | SWP_SHOWWINDOW);
    SetWindowRgn(hwnd, CreateRoundRectRgn(0, 0, geometry.width + 1, geometry.height + 1,
                                         radius, radius), TRUE);
}

static void cclover_add_tray(HWND hwnd, CcloverHost *host) {
    NOTIFYICONDATAW *tray = &host->tray;
    ZeroMemory(tray, sizeof(*tray));
#ifdef NOTIFYICONDATA_V2_SIZE
    tray->cbSize = NOTIFYICONDATA_V2_SIZE;
#else
    tray->cbSize = sizeof(*tray);
#endif
    tray->hWnd = hwnd;
    tray->uID = 1;
    tray->uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    tray->uCallbackMessage = CCLOVER_WM_TRAY;
    tray->hIcon = LoadIconW(NULL, IDI_APPLICATION);
    lstrcpynW(tray->szTip, L"cclover-mon", (int)(sizeof(tray->szTip) / sizeof(tray->szTip[0])));
    Shell_NotifyIconW(NIM_ADD, tray);
}

static void cclover_tray_menu(HWND hwnd) {
    POINT point;
    HMENU menu = CreatePopupMenu();
    UINT command;
    if (!menu) return;
    AppendMenuW(menu, MF_STRING, CCLOVER_MENU_QUIT, L"Quit");
    GetCursorPos(&point);
    SetForegroundWindow(hwnd);
    command = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON,
                             point.x, point.y, 0, hwnd, NULL);
    DestroyMenu(menu);
    if (command == CCLOVER_MENU_QUIT) DestroyWindow(hwnd);
    PostMessageW(hwnd, WM_NULL, 0, 0);
}

static LRESULT CALLBACK cclover_wndproc(HWND hwnd, UINT message, WPARAM wparam, LPARAM lparam) {
    CcloverHost *host = (CcloverHost *)GetWindowLongPtrW(hwnd, GWLP_USERDATA);
    if (message == WM_NCCREATE) {
        CREATESTRUCTW *create = (CREATESTRUCTW *)lparam;
        host = (CcloverHost *)create->lpCreateParams;
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, (LONG_PTR)host);
    }
    if (!host) return DefWindowProcW(hwnd, message, wparam, lparam);

    if (host->taskbar_created && message == host->taskbar_created) {
        CcloverScene scene;
        cclover_refresh_display(host, hwnd);
        cclover_scene(host, &scene);
        cclover_place(host, hwnd, scene.width, scene.height);
        cclover_add_tray(hwnd, host);
        InvalidateRect(hwnd, NULL, FALSE);
        return 0;
    }

    switch (message) {
    case WM_DPICHANGED: {
        const RECT *suggested = (const RECT *)lparam;
        HMONITOR monitor = MonitorFromRect(suggested, MONITOR_DEFAULTTONEAREST);
        CcloverScene scene;
        cclover_set_display(host, monitor, LOWORD(wparam));
        cclover_scene(host, &scene);
        cclover_place(host, hwnd, scene.width, scene.height);
        InvalidateRect(hwnd, NULL, FALSE);
        return 0;
    }
    case CCLOVER_WM_STATE: {
        uint32_t state = host->callbacks->take_state(host->context);
        if (state & CCLOVER_STATE_CHANGED) {
            CcloverScene scene;
            cclover_refresh_display(host, hwnd);
            cclover_scene(host, &scene);
            cclover_place(host, hwnd, scene.width, scene.height);
            InvalidateRect(hwnd, NULL, FALSE);
        }
        return 0;
    }
    case WM_PAINT: {
        PAINTSTRUCT paint;
        CcloverScene scene;
        HDC target = BeginPaint(hwnd, &paint);
        HDC buffer = CreateCompatibleDC(target);
        HBITMAP bitmap;
        HGDIOBJ old_bitmap;
        RECT client;
        GetClientRect(hwnd, &client);
        bitmap = CreateCompatibleBitmap(target, client.right, client.bottom);
        old_bitmap = SelectObject(buffer, bitmap);
        FillRect(buffer, &client, (HBRUSH)GetStockObject(BLACK_BRUSH));
        cclover_scene(host, &scene);
        cclover_draw_scene(host, buffer, &scene);
        BitBlt(target, 0, 0, client.right, client.bottom, buffer, 0, 0, SRCCOPY);
        SelectObject(buffer, old_bitmap);
        DeleteObject(bitmap);
        DeleteDC(buffer);
        EndPaint(hwnd, &paint);
        return 0;
    }
    case WM_NCHITTEST:
        return HTTRANSPARENT;
    case WM_CLOSE:
        DestroyWindow(hwnd);
        return 0;
    case WM_COMMAND:
        if (LOWORD(wparam) == CCLOVER_MENU_QUIT) DestroyWindow(hwnd);
        return 0;
    case CCLOVER_WM_TRAY:
        if (lparam == WM_RBUTTONUP || lparam == WM_CONTEXTMENU) cclover_tray_menu(hwnd);
        return 0;
    case WM_DESTROY: {
        Shell_NotifyIconW(NIM_DELETE, &host->tray);
        cclover_release_resources(host);
        PostQuitMessage(0);
        return 0;
    }
    default:
        return DefWindowProcW(hwnd, message, wparam, lparam);
    }
}

DWORD cclover_win32_prepare_wake(void) {
    MSG message;
    PeekMessageW(&message, NULL, WM_USER, WM_USER, PM_NOREMOVE);
    return GetCurrentThreadId();
}

void cclover_win32_wake(DWORD thread_id) {
    PostThreadMessageW(thread_id, CCLOVER_WM_STATE, 0, 0);
}

int cclover_win32_run(void *context, const CcloverCallbacks *callbacks) {
    static const wchar_t CLASS_NAME[] = L"CcloverMonNativeWindow";
    HINSTANCE instance = GetModuleHandleW(NULL);
    WNDCLASSEXW window_class;
    CcloverHost host;
    CcloverScene scene;
    HWND shell;
    HWND hwnd;
    MSG message;

    ZeroMemory(&host, sizeof(host));
    host.context = context;
    host.callbacks = callbacks;
    cclover_init_dpi_api(&host.dpi_api);
    cclover_enable_dpi_awareness(&host.dpi_api);
    cclover_refresh_display(&host, NULL);
    host.measure_dc = CreateCompatibleDC(NULL);
    if (!host.measure_dc) {
        int error = (int)GetLastError();
        cclover_release_resources(&host);
        return error;
    }
    host.taskbar_created = RegisterWindowMessageW(L"TaskbarCreated");
    cclover_scene(&host, &scene);
    shell = GetShellWindow();

    ZeroMemory(&window_class, sizeof(window_class));
    window_class.cbSize = sizeof(window_class);
    window_class.lpfnWndProc = cclover_wndproc;
    window_class.hInstance = instance;
    window_class.hCursor = LoadCursorW(NULL, IDC_ARROW);
    window_class.lpszClassName = CLASS_NAME;
    if (!RegisterClassExW(&window_class) && GetLastError() != ERROR_CLASS_ALREADY_EXISTS) {
        int error = (int)GetLastError();
        cclover_release_resources(&host);
        return error;
    }

    hwnd = CreateWindowExW(
        WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED | WS_EX_TRANSPARENT,
        CLASS_NAME, L"cclover-mon", WS_POPUP,
        0, 0, cclover_px(&host, (float)scene.width),
        cclover_px(&host, (float)scene.height),
        shell, NULL, instance, &host);
    if (!hwnd) {
        int error = (int)GetLastError();
        cclover_release_resources(&host);
        return error;
    }

    cclover_refresh_display(&host, hwnd);
    cclover_scene(&host, &scene);

    if (!SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA)) {
        int error = (int)GetLastError();
        DestroyWindow(hwnd);
        return error;
    }

    cclover_place(&host, hwnd, scene.width, scene.height);
    cclover_add_tray(hwnd, &host);
    InvalidateRect(hwnd, NULL, FALSE);

    while (GetMessageW(&message, NULL, 0, 0) > 0) {
        if (message.hwnd == NULL && message.message == CCLOVER_WM_STATE) {
            PostMessageW(hwnd, CCLOVER_WM_STATE, 0, 0);
            continue;
        }
        TranslateMessage(&message);
        DispatchMessageW(&message);
    }
    return 0;
}
