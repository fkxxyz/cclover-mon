#define UNICODE
#define _UNICODE
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <shellapi.h>
#include <stdint.h>
#include <stdio.h>

#include "native_scene.h"
#include "display.h"
#include "drawing.h"
#include "layered.h"
#include "message_policy.h"
#include "windows_geometry.h"

#define CCLOVER_WM_TRAY (WM_APP + 7)
#define CCLOVER_WM_STATE (WM_APP + 8)
#define CCLOVER_MENU_QUIT 1001
#define IDI_CCLOVER_MON 101
#define CCLOVER_MARGIN 16
#ifndef WM_DPICHANGED
#define WM_DPICHANGED 0x02E0
#endif

typedef struct {
    void *context;
    const CcloverCallbacks *callbacks;
    NOTIFYICONDATAW tray;
    HICON icon_large;
    HICON icon_small;
    UINT taskbar_created;
    CcloverDisplay display;
    CcloverGdiRenderer renderer;
    CcloverLayeredSurface surface;
    int present_failure_reported;
} CcloverHost;

static void cclover_scene(CcloverHost *host, CcloverScene *scene) {
    ZeroMemory(scene, sizeof(*scene));
    host->callbacks->scene(host->context, scene);
}

static void cclover_place(CcloverHost *host, HWND hwnd, uint32_t width, uint32_t height) {
    HWND shell = GetShellWindow();
    RECT display_work = cclover_display_work_area(&host->display);
    CcloverRectI work = {
        display_work.left,
        display_work.top,
        display_work.right,
        display_work.bottom,
    };
    CcloverWindowGeometry geometry = cclover_top_right_geometry(
        width, height, work, cclover_display_dpi(&host->display), (float)CCLOVER_MARGIN);
    if (shell && GetWindow(hwnd, GW_OWNER) != shell) {
        SetWindowLongPtrW(hwnd, GWLP_HWNDPARENT, (LONG_PTR)shell);
    }
    SetWindowPos(hwnd, HWND_BOTTOM, geometry.x, geometry.y, geometry.width, geometry.height,
                 SWP_NOACTIVATE | SWP_SHOWWINDOW);
}

/*
 * Realizes the current Scene on the layered window with per-pixel alpha and returns a
 * CCLOVER_LAYERED_* status. A rejected or failed frame leaves the previously published
 * frame on screen; a publication failure is reported once until the next success.
 */
static int cclover_present(CcloverHost *host, HWND hwnd) {
    CcloverScene scene;
    RECT bounds;
    int status;
    if (!GetWindowRect(hwnd, &bounds)) {
        host->surface.last_error = GetLastError();
        status = CCLOVER_LAYERED_FAILED;
    } else {
        cclover_scene(host, &scene);
        status = cclover_layered_present(&host->surface, &host->renderer, hwnd, &scene,
                                         cclover_display_dpi(&host->display),
                                         bounds.right - bounds.left,
                                         bounds.bottom - bounds.top);
    }
    if (status == CCLOVER_LAYERED_OK) {
        host->present_failure_reported = 0;
    } else if (status == CCLOVER_LAYERED_FAILED && !host->present_failure_reported) {
        char message[96];
        snprintf(message, sizeof(message),
                 "cclover-mon: layered surface publication failed (error %lu)\n",
                 (unsigned long)host->surface.last_error);
        fputs(message, stderr);
        OutputDebugStringA(message);
        host->present_failure_reported = 1;
    }
    return status;
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
    tray->hIcon = host->icon_small;
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
        cclover_display_refresh(&host->display, hwnd);
        cclover_scene(host, &scene);
        cclover_place(host, hwnd, scene.width, scene.height);
        cclover_add_tray(hwnd, host);
        cclover_present(host, hwnd);
        return 0;
    }

    switch (message) {
    case WM_DPICHANGED: {
        const RECT *suggested = (const RECT *)lparam;
        HMONITOR monitor = MonitorFromRect(suggested, MONITOR_DEFAULTTONEAREST);
        CcloverScene scene;
        cclover_display_set(&host->display, monitor, LOWORD(wparam));
        cclover_scene(host, &scene);
        cclover_place(host, hwnd, scene.width, scene.height);
        cclover_present(host, hwnd);
        return 0;
    }
    case CCLOVER_WM_STATE: {
        uint32_t state = host->callbacks->take_state(host->context);
        if (cclover_win32_state_requires_refresh(state, CCLOVER_STATE_CHANGED)) {
            CcloverScene scene;
            cclover_display_refresh(&host->display, hwnd);
            cclover_scene(host, &scene);
            cclover_place(host, hwnd, scene.width, scene.height);
            cclover_present(host, hwnd);
        }
        return 0;
    }
    case WM_PAINT: {
        /* Content is published through UpdateLayeredWindow; only validate. */
        PAINTSTRUCT paint;
        BeginPaint(hwnd, &paint);
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
        if (host->icon_large) DestroyIcon(host->icon_large);
        if (host->icon_small) DestroyIcon(host->icon_small);
        cclover_layered_destroy(&host->surface);
        cclover_gdi_renderer_destroy(&host->renderer);
        cclover_display_destroy(&host->display);
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
    cclover_display_init(&host.display);
    cclover_display_enable_dpi_awareness(&host.display);
    cclover_display_refresh(&host.display, NULL);
    host.taskbar_created = RegisterWindowMessageW(L"TaskbarCreated");
    cclover_scene(&host, &scene);
    shell = GetShellWindow();

    ZeroMemory(&window_class, sizeof(window_class));
    window_class.cbSize = sizeof(window_class);
    window_class.lpfnWndProc = cclover_wndproc;
    window_class.hInstance = instance;
    window_class.hCursor = LoadCursorW(NULL, IDC_ARROW);
    host.icon_large = (HICON)LoadImageW(instance, MAKEINTRESOURCEW(IDI_CCLOVER_MON), IMAGE_ICON,
                                        GetSystemMetrics(SM_CXICON), GetSystemMetrics(SM_CYICON),
                                        LR_DEFAULTCOLOR);
    host.icon_small = (HICON)LoadImageW(instance, MAKEINTRESOURCEW(IDI_CCLOVER_MON), IMAGE_ICON,
                                        GetSystemMetrics(SM_CXSMICON), GetSystemMetrics(SM_CYSMICON),
                                        LR_DEFAULTCOLOR);
    if (!host.icon_large || !host.icon_small) {
        int error = (int)GetLastError();
        if (host.icon_large) DestroyIcon(host.icon_large);
        if (host.icon_small) DestroyIcon(host.icon_small);
        cclover_gdi_renderer_destroy(&host.renderer);
        cclover_display_destroy(&host.display);
        return error;
    }
    window_class.hIcon = host.icon_large;
    window_class.hIconSm = host.icon_small;
    window_class.lpszClassName = CLASS_NAME;
    if (!RegisterClassExW(&window_class) && GetLastError() != ERROR_CLASS_ALREADY_EXISTS) {
        int error = (int)GetLastError();
        DestroyIcon(host.icon_large);
        DestroyIcon(host.icon_small);
        cclover_gdi_renderer_destroy(&host.renderer);
        cclover_display_destroy(&host.display);
        return error;
    }

    hwnd = CreateWindowExW(
        WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED | WS_EX_TRANSPARENT,
        CLASS_NAME, L"cclover-mon", WS_POPUP,
        0, 0, cclover_display_px(&host.display, (float)scene.width),
        cclover_display_px(&host.display, (float)scene.height),
        shell, NULL, instance, &host);
    if (!hwnd) {
        int error = (int)GetLastError();
        DestroyIcon(host.icon_large);
        DestroyIcon(host.icon_small);
        cclover_gdi_renderer_destroy(&host.renderer);
        cclover_display_destroy(&host.display);
        return error;
    }

    cclover_display_refresh(&host.display, hwnd);
    cclover_scene(&host, &scene);

    cclover_place(&host, hwnd, scene.width, scene.height);
    if (cclover_present(&host, hwnd) == CCLOVER_LAYERED_FAILED) {
        int error = (int)host.surface.last_error;
        DestroyWindow(hwnd);
        return error;
    }
    cclover_add_tray(hwnd, &host);

    while (GetMessageW(&message, NULL, 0, 0) > 0) {
        if (cclover_win32_should_relay_state(
                message.hwnd != NULL, message.message, CCLOVER_WM_STATE)) {
            PostMessageW(hwnd, CCLOVER_WM_STATE, 0, 0);
            continue;
        }
        TranslateMessage(&message);
        DispatchMessageW(&message);
    }
    return 0;
}
