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

