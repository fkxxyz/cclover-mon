#define UNICODE
#define _UNICODE
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <shellapi.h>
#include <stdint.h>
#include <stddef.h>
#include "native_scene.h"

#define CCLOVER_WM_TRAY (WM_APP + 7)
#define CCLOVER_TIMER_ID 1
#define CCLOVER_MENU_QUIT 1001
#define CCLOVER_MARGIN 16

typedef struct {
    void *context;
    const CcloverCallbacks *callbacks;
    NOTIFYICONDATAW tray;
    HFONT fonts[32][2];
    UINT taskbar_created;
} CcloverHost;

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

static int cclover_round(float value) { return (int)(value + 0.5f); }

static HFONT cclover_font(CcloverHost *host, HDC dc, uint32_t size, int bold) {
    uint32_t slot = size < 32 ? size : 31;
    HFONT cached = host->fonts[slot][bold ? 1 : 0];
    if (cached) return cached;
    int pixels = -MulDiv((int)size, GetDeviceCaps(dc, LOGPIXELSY), 96);
    if (pixels == 0) pixels = -(int)size;
    cached = CreateFontW(pixels, 0, 0, 0, bold ? FW_BOLD : FW_NORMAL,
        FALSE, FALSE, FALSE, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS,
        CLEARTYPE_QUALITY, FIXED_PITCH | FF_MODERN, L"Inconsolata");
    host->fonts[slot][bold ? 1 : 0] = cached;
    return cached;
}

static void cclover_draw_rect(HDC dc, const CcloverCommand *cmd, int fill) {
    int left = cclover_round(cmd->x), top = cclover_round(cmd->y);
    int right = cclover_round(cmd->x + cmd->width), bottom = cclover_round(cmd->y + cmd->height);
    int diameter = cclover_round(cmd->radius * 2.0f);
    HGDIOBJ old_brush, old_pen;
    HBRUSH brush = fill ? CreateSolidBrush(cclover_color(cmd->color)) : (HBRUSH)GetStockObject(NULL_BRUSH);
    HPEN pen = fill ? (HPEN)GetStockObject(NULL_PEN)
                    : CreatePen(PS_SOLID, max(1, cclover_round(cmd->stroke_width)), cclover_color(cmd->color));
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
    HFONT font = cclover_font(host, dc, cmd->text_size,
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
    rect.left = cclover_round(cmd->x);
    rect.top = cclover_round(cmd->y);
    rect.right = cclover_round(cmd->x + cmd->width);
    rect.bottom = cclover_round(cmd->y + cmd->height);
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
            cclover_draw_rect(dc, cmd, 1);
        } else if (cmd->kind == CCLOVER_CMD_STROKE_RECT) {
            cclover_draw_rect(dc, cmd, 0);
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
                points[j].x = cclover_round(source->x);
                points[j].y = cclover_round(source->y);
            }
            if (cmd->kind == CCLOVER_CMD_POLYLINE) {
                HPEN pen = CreatePen(PS_SOLID, max(1, cclover_round(cmd->stroke_width)), cclover_color(cmd->color));
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
    host->callbacks->scene(host->context, scene);
}

static void cclover_place(HWND hwnd, uint32_t width, uint32_t height) {
    HWND shell = GetShellWindow();
    RECT work;
    int x, y;
    if (shell && GetWindow(hwnd, GW_OWNER) != shell) {
        SetWindowLongPtrW(hwnd, GWLP_HWNDPARENT, (LONG_PTR)shell);
    }
    if (!SystemParametersInfoW(SPI_GETWORKAREA, 0, &work, 0)) {
        work.left = 0;
        work.top = 0;
        work.right = GetSystemMetrics(SM_CXSCREEN);
        work.bottom = GetSystemMetrics(SM_CYSCREEN);
    }
    x = work.right - (int)width - CCLOVER_MARGIN;
    y = work.top + CCLOVER_MARGIN;
    if (x < work.left) x = work.left;
    SetWindowPos(hwnd, HWND_BOTTOM, x, y, (int)width, (int)height,
                 SWP_NOACTIVATE | SWP_SHOWWINDOW);
    SetWindowRgn(hwnd, CreateRoundRectRgn(0, 0, (int)width + 1, (int)height + 1, 32, 32), TRUE);
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
        cclover_scene(host, &scene);
        cclover_place(hwnd, scene.width, scene.height);
        cclover_add_tray(hwnd, host);
        InvalidateRect(hwnd, NULL, FALSE);
        return 0;
    }

    switch (message) {
    case WM_TIMER: {
        if (wparam == CCLOVER_TIMER_ID) {
            uint32_t poll = host->callbacks->poll(host->context);
            if (poll & CCLOVER_POLL_QUIT) {
                DestroyWindow(hwnd);
                return 0;
            }
            if (poll & CCLOVER_POLL_FRAME) {
                CcloverScene scene;
                cclover_scene(host, &scene);
                cclover_place(hwnd, scene.width, scene.height);
                InvalidateRect(hwnd, NULL, FALSE);
            }
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
        unsigned i, j;
        KillTimer(hwnd, CCLOVER_TIMER_ID);
        Shell_NotifyIconW(NIM_DELETE, &host->tray);
        for (i = 0; i < 32; ++i) {
            for (j = 0; j < 2; ++j) {
                if (host->fonts[i][j]) DeleteObject(host->fonts[i][j]);
            }
        }
        PostQuitMessage(0);
        return 0;
    }
    default:
        return DefWindowProcW(hwnd, message, wparam, lparam);
    }
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
    host.taskbar_created = RegisterWindowMessageW(L"TaskbarCreated");
    cclover_scene(&host, &scene);
    shell = GetShellWindow();

    ZeroMemory(&window_class, sizeof(window_class));
    window_class.cbSize = sizeof(window_class);
    window_class.lpfnWndProc = cclover_wndproc;
    window_class.hInstance = instance;
    window_class.hCursor = LoadCursorW(NULL, IDC_ARROW);
    window_class.lpszClassName = CLASS_NAME;
    if (!RegisterClassExW(&window_class) && GetLastError() != ERROR_CLASS_ALREADY_EXISTS)
        return (int)GetLastError();

    hwnd = CreateWindowExW(
        WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED | WS_EX_TRANSPARENT,
        CLASS_NAME, L"cclover-mon", WS_POPUP,
        0, 0, (int)scene.width, (int)scene.height,
        shell, NULL, instance, &host);
    if (!hwnd) return (int)GetLastError();

    if (!SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA)) {
        int error = (int)GetLastError();
        DestroyWindow(hwnd);
        return error;
    }

    cclover_place(hwnd, scene.width, scene.height);
    cclover_add_tray(hwnd, &host);
    SetTimer(hwnd, CCLOVER_TIMER_ID, 100, NULL);
    InvalidateRect(hwnd, NULL, FALSE);

    while (GetMessageW(&message, NULL, 0, 0) > 0) {
        TranslateMessage(&message);
        DispatchMessageW(&message);
    }
    return 0;
}
