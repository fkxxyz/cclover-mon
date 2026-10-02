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
    host->callbacks->scene(host->context, scene);
}

