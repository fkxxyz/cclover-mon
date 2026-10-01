static double cclover_profile_cpu_ms(void) {
    struct timespec ts;
    clock_gettime(CLOCK_THREAD_CPUTIME_ID, &ts);
    return (double)ts.tv_sec * 1000.0 + (double)ts.tv_nsec / 1000000.0;
}

typedef struct {
    uint64_t hash;
    uint8_t *text;
    size_t text_len;
    uint32_t size;
    uint32_t flags;
    float width;
} CcloverTextMeasureCacheEntry;

typedef struct {
    uint32_t size;
    uint32_t bold;
    cairo_font_extents_t extents;
    int valid;
} CcloverFontMetricsCacheEntry;

typedef struct {
    void *context;
    const CcloverCallbacks *callbacks;
    cairo_surface_t *measure_surface;
    cairo_t *measure_cr;
    CcloverTextMeasureCacheEntry measure_cache[CCLOVER_TEXT_MEASURE_CACHE_SIZE];
    CcloverFontMetricsCacheEntry font_metrics[32];
} CcloverHost;

static void cclover_source_argb(cairo_t *cr, uint32_t argb) {
    double a = ((argb >> 24) & 0xff) / 255.0;
    double r = ((argb >> 16) & 0xff) / 255.0;
    double g = ((argb >> 8) & 0xff) / 255.0;
    double b = (argb & 0xff) / 255.0;
    cairo_set_source_rgba(cr, r, g, b, a);
}

static void cclover_round_rect(cairo_t *cr, const CcloverCommand *cmd) {
    double x = cmd->x, y = cmd->y, w = cmd->width, h = cmd->height;
    double r = fmin(cmd->radius, fmin(w, h) / 2.0);
    if (r <= 0.0) {
        cairo_rectangle(cr, x, y, w, h);
        return;
    }
    cairo_new_sub_path(cr);
    cairo_arc(cr, x + w - r, y + r, r, -M_PI / 2.0, 0.0);
    cairo_arc(cr, x + w - r, y + h - r, r, 0.0, M_PI / 2.0);
    cairo_arc(cr, x + r, y + h - r, r, M_PI / 2.0, M_PI);
    cairo_arc(cr, x + r, y + r, r, M_PI, M_PI * 3.0 / 2.0);
    cairo_close_path(cr);
}

static char *cclover_text_copy(const uint8_t *bytes, size_t len, char stack[512]) {
    char *text = stack;
    if (len + 1 > 512) {
        text = malloc(len + 1);
        if (!text) return NULL;
    }
    memcpy(text, bytes, len);
    text[len] = '\0';
    return text;
}

static void cclover_configure_context(cairo_t *cr) {
    cairo_font_options_t *font_options = cairo_font_options_create();
    cairo_font_options_set_hint_metrics(font_options, CAIRO_HINT_METRICS_OFF);
    cairo_set_font_options(cr, font_options);
    cairo_font_options_destroy(font_options);
}

static void cclover_select_font(cairo_t *cr, uint32_t size, int bold) {
    cairo_select_font_face(cr, "Inconsolata", CAIRO_FONT_SLANT_NORMAL,
                           bold ? CAIRO_FONT_WEIGHT_BOLD : CAIRO_FONT_WEIGHT_NORMAL);
    cairo_set_font_size(cr, size);
}

static int cclover_measure_init(CcloverHost *host) {
    host->measure_surface = cairo_image_surface_create(CAIRO_FORMAT_ARGB32, 1, 1);
    if (cairo_surface_status(host->measure_surface) != CAIRO_STATUS_SUCCESS) return -1;
    host->measure_cr = cairo_create(host->measure_surface);
    if (cairo_status(host->measure_cr) != CAIRO_STATUS_SUCCESS) return -1;
    cclover_configure_context(host->measure_cr);
    return 0;
}

static void cclover_measure_destroy(CcloverHost *host) {
    size_t i;
    for (i = 0; i < CCLOVER_TEXT_MEASURE_CACHE_SIZE; ++i) {
        free(host->measure_cache[i].text);
        host->measure_cache[i].text = NULL;
    }
    if (host->measure_cr) cairo_destroy(host->measure_cr);
    if (host->measure_surface) cairo_surface_destroy(host->measure_surface);
    host->measure_cr = NULL;
    host->measure_surface = NULL;
}

static uint64_t cclover_measure_hash(const uint8_t *bytes, size_t len,
                                     uint32_t size, uint32_t flags) {
    uint64_t hash = UINT64_C(1469598103934665603);
    size_t i;
    for (i = 0; i < len; ++i) {
        hash ^= bytes[i];
        hash *= UINT64_C(1099511628211);
    }
    hash ^= size;
    hash *= UINT64_C(1099511628211);
    hash ^= flags;
    hash *= UINT64_C(1099511628211);
    return hash;
}

static float cclover_measure_text(void *context, const uint8_t *bytes, size_t len,
                                  uint32_t size, uint32_t flags) {
    CcloverHost *host = context;
    uint64_t hash = cclover_measure_hash(bytes, len, size, flags);
    CcloverTextMeasureCacheEntry *cached =
        &host->measure_cache[hash % CCLOVER_TEXT_MEASURE_CACHE_SIZE];
    char stack[512];
    char *text;
    cairo_text_extents_t ext;
    if (cached->text && cached->hash == hash && cached->text_len == len &&
        cached->size == size && cached->flags == flags &&
        memcmp(cached->text, bytes, len) == 0)
        return cached->width;
    text = cclover_text_copy(bytes, len, stack);
    if (!text) return 0.0f;
    cclover_select_font(host->measure_cr, size, (flags & CCLOVER_TEXT_BOLD) != 0);
    cairo_text_extents(host->measure_cr, text, &ext);
    if (len > 0) {
        uint8_t *copy = malloc(len);
        if (copy) {
            memcpy(copy, bytes, len);
            free(cached->text);
            cached->hash = hash;
            cached->text = copy;
            cached->text_len = len;
            cached->size = size;
            cached->flags = flags;
            cached->width = (float)ext.x_advance;
        }
    }
    if (text != stack) free(text);
    return (float)ext.x_advance;
}


static void cclover_font_extents(CcloverHost *host, cairo_t *cr, uint32_t size,
                                 uint32_t bold, cairo_font_extents_t *extents) {
    CcloverFontMetricsCacheEntry *cached =
        &host->font_metrics[(size * 2u + bold) % 32u];
    if (cached->valid && cached->size == size && cached->bold == bold) {
        *extents = cached->extents;
        return;
    }
    cairo_font_extents(cr, extents);
    cached->size = size;
    cached->bold = bold;
    cached->extents = *extents;
    cached->valid = 1;
}

static void cclover_draw_text(CcloverHost *host, cairo_t *cr, const CcloverCommand *cmd,
                              uint32_t *font_size, uint32_t *font_bold, int *font_valid) {
    char stack[512];
    char *text = cclover_text_copy(cmd->text, cmd->text_len, stack);
    cairo_font_extents_t font;
    uint32_t bold;
    int clipped;
    double x, y;
    if (!text) return;

    bold = (cmd->flags & CCLOVER_TEXT_BOLD) != 0;
    if (!*font_valid || *font_size != cmd->text_size || *font_bold != bold) {
        cclover_select_font(cr, cmd->text_size, bold);
        *font_size = cmd->text_size;
        *font_bold = bold;
        *font_valid = 1;
    }
    clipped = (cmd->flags & CCLOVER_TEXT_CLIP) != 0;
    if (clipped) {
        cairo_save(cr);
        cairo_rectangle(cr, cmd->x, cmd->y, cmd->width, cmd->height);
        cairo_clip(cr);
    }
    cclover_font_extents(host, cr, cmd->text_size, bold, &font);
    /* NativeScene gives end-aligned cells their measured natural width, so the
       rect's left edge is already the final text origin. */
    x = cmd->x;
    y = cmd->y + (cmd->height - (font.ascent + font.descent)) / 2.0 + font.ascent;
    cclover_source_argb(cr, cmd->color);
    cairo_move_to(cr, x, y);
    cairo_show_text(cr, text);
    if (clipped) cairo_restore(cr);
    if (text != stack) free(text);
}

#define CCLOVER_DRAW_ALL 0
#define CCLOVER_DRAW_STATIC 1
#define CCLOVER_DRAW_DYNAMIC 2

static int cclover_draw_command(int mode, const CcloverCommand *cmd) {
    int is_static = (cmd->flags & CCLOVER_STATIC_CONTENT) != 0;
    return mode == CCLOVER_DRAW_ALL ||
           (mode == CCLOVER_DRAW_STATIC && is_static) ||
           (mode == CCLOVER_DRAW_DYNAMIC && !is_static);
}

static void cclover_draw_scene(CcloverHost *host, cairo_t *cr, const CcloverScene *scene,
                               int clear, int mode) {
    size_t i, j;
    uint32_t font_size = 0;
    uint32_t font_bold = 0;
    int font_valid = 0;
    cairo_save(cr);
    if (clear) {
        cairo_set_operator(cr, CAIRO_OPERATOR_SOURCE);
        cairo_set_source_rgba(cr, 0, 0, 0, 0);
        cairo_paint(cr);
    }
    cairo_set_operator(cr, CAIRO_OPERATOR_OVER);

    for (i = 0; i < scene->command_count; ++i) {
        const CcloverCommand *cmd = &scene->commands[i];
        if (!cclover_draw_command(mode, cmd)) continue;
        if (cmd->kind == CCLOVER_CMD_FILL_RECT) {
            cclover_round_rect(cr, cmd);
            cclover_source_argb(cr, cmd->color);
            cairo_fill(cr);
        } else if (cmd->kind == CCLOVER_CMD_STROKE_RECT) {
            cclover_round_rect(cr, cmd);
            cclover_source_argb(cr, cmd->color);
            cairo_set_line_width(cr, cmd->stroke_width);
            cairo_stroke(cr);
        } else if (cmd->kind == CCLOVER_CMD_TEXT) {
            cclover_draw_text(host, cr, cmd, &font_size, &font_bold, &font_valid);
        } else if ((cmd->kind == CCLOVER_CMD_POLYLINE ||
                    cmd->kind == CCLOVER_CMD_POLYGON) &&
                   cmd->point_count > 0 &&
                   cmd->point_offset + cmd->point_count <= scene->point_count) {
            const CcloverPoint *first = &scene->points[cmd->point_offset];
            cairo_move_to(cr, first->x, first->y);
            for (j = 1; j < cmd->point_count; ++j) {
                const CcloverPoint *point = &scene->points[cmd->point_offset + j];
                cairo_line_to(cr, point->x, point->y);
            }
            cclover_source_argb(cr, cmd->color);
            if (cmd->kind == CCLOVER_CMD_POLYGON) {
                cairo_close_path(cr);
                cairo_fill(cr);
            } else {
                cairo_set_line_width(cr, cmd->stroke_width);
                cairo_stroke(cr);
            }
        }
    }
    cairo_restore(cr);
}

static void cclover_scene(CcloverHost *host, CcloverScene *scene) {
    memset(scene, 0, sizeof(*scene));
    host->callbacks->scene(host->context, host, cclover_measure_text, scene);
}

