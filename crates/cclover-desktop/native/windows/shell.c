#define UNICODE
#define _UNICODE
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <shellapi.h>
#include <stdint.h>

#include "native_scene.h"
#include "display.h"
#include "drawing.h"
#include "message_policy.h"
#include "windows_geometry.h"

#define CCLOVER_WM_TRAY (WM_APP + 7)
#define CCLOVER_WM_STATE (WM_APP + 8)
#define CCLOVER_MENU_QUIT 1001
#define CCLOVER_MARGIN 16
#ifndef WM_DPICHANGED
#define WM_DPICHANGED 0x02E0
#endif

typedef struct {
    void *context;
    const CcloverCallbacks *callbacks;
    NOTIFYICONDATAW tray;
    UINT taskbar_created;
    CcloverDisplay display;
    CcloverGdiRenderer renderer;
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
    int radius = cclover_display_px(&host->display, 32.0f);
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
        cclover_display_refresh(&host->display, hwnd);
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
        cclover_display_set(&host->display, monitor, LOWORD(wparam));
        cclover_scene(host, &scene);
        cclover_place(host, hwnd, scene.width, scene.height);
        InvalidateRect(hwnd, NULL, FALSE);
        return 0;
    }
    case CCLOVER_WM_STATE: {
        uint32_t state = host->callbacks->take_state(host->context);
        if (cclover_win32_state_requires_refresh(state, CCLOVER_STATE_CHANGED)) {
            CcloverScene scene;
            cclover_display_refresh(&host->display, hwnd);
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
        if (cclover_gdi_draw_scene(&host->renderer, buffer, &scene,
                                   cclover_display_dpi(&host->display)) == 0)
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
    window_class.lpszClassName = CLASS_NAME;
    if (!RegisterClassExW(&window_class) && GetLastError() != ERROR_CLASS_ALREADY_EXISTS) {
        int error = (int)GetLastError();
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
        cclover_gdi_renderer_destroy(&host.renderer);
        cclover_display_destroy(&host.display);
        return error;
    }

    cclover_display_refresh(&host.display, hwnd);
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
