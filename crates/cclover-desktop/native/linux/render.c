#include "render.h"

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

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

void cclover_cairo_configure_context(cairo_t *cr) {
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

static void cclover_ensure_font_selected(cairo_t *cr, uint32_t size, uint32_t bold,
                                         uint32_t *font_size, uint32_t *font_bold,
                                         int *font_valid) {
    if (*font_valid && *font_size == size && *font_bold == bold) return;
    cclover_select_font(cr, size, (int)bold);
    *font_size = size;
    *font_bold = bold;
    *font_valid = 1;
}

static void cclover_font_extents(CcloverCairoRenderer *renderer, cairo_t *cr, uint32_t size,
                                 uint32_t bold, cairo_font_extents_t *extents) {
    CcloverFontMetricsCacheEntry *cached =
        &renderer->font_metrics[(size * 2u + bold) % 32u];
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

static void cclover_draw_text(CcloverCairoRenderer *renderer, cairo_t *cr, const CcloverCommand *cmd,
                              const CcloverTextAdvance *prepared_advance,
                              uint32_t *font_size, uint32_t *font_bold, int *font_valid) {
    char stack[512];
    char *text = cclover_text_copy(cmd->text, cmd->text_len, stack);
    cairo_font_extents_t font;
    cairo_text_extents_t text_extents;
    uint32_t bold;
    int clipped;
    double x, y;
    if (!text) return;

    bold = (cmd->flags & CCLOVER_TEXT_BOLD) != 0;
    cclover_ensure_font_selected(cr, cmd->text_size, bold,
                                 font_size, font_bold, font_valid);
    clipped = (cmd->flags & CCLOVER_TEXT_CLIP) != 0;
    if (clipped) {
        cairo_save(cr);
        cairo_rectangle(cr, cmd->x, cmd->y, cmd->width, cmd->height);
        cairo_clip(cr);
    }
    cclover_font_extents(renderer, cr, cmd->text_size, bold, &font);
    x = cmd->x;
    if (cmd->flags & CCLOVER_TEXT_END) {
        double x_advance;
        if (prepared_advance && prepared_advance->valid) {
            x_advance = prepared_advance->x_advance;
        } else {
            cairo_text_extents(cr, text, &text_extents);
            x_advance = text_extents.x_advance;
        }
        x = cmd->x + cmd->width - x_advance;
    }
    y = cmd->y + (cmd->height - (font.ascent + font.descent)) / 2.0 + font.ascent;
    cclover_source_argb(cr, cmd->color);
    cairo_move_to(cr, x, y);
    cairo_show_text(cr, text);
    if (clipped) cairo_restore(cr);
    if (text != stack) free(text);
}

static int cclover_draw_command(const CcloverScene *scene,
                                CcloverCairoDrawMode mode,
                                size_t command_index,
                                int cull_to_redraw_mask,
                                int redraw_mask_valid) {
    const CcloverCommand *cmd = &scene->commands[command_index];
    int is_static = (cmd->flags & CCLOVER_STATIC_CONTENT) != 0;
    int mask_value = redraw_mask_valid ? scene->redraw_mask[command_index] : 1;
    return cclover_cairo_command_selected(mode, is_static, cull_to_redraw_mask,
                                          redraw_mask_valid, mask_value);
}

static int cclover_redraw_mask_valid(const CcloverScene *scene,
                                     int cull_to_redraw_mask) {
    return cull_to_redraw_mask && scene->redraw_mask != NULL &&
           scene->redraw_mask_count == scene->command_count;
}

static int cclover_cairo_text_fits(cairo_t *cr, const CcloverCommand *cmd,
                                   double *x_advance) {
    char stack[512];
    char *text = cclover_text_copy(cmd->text, cmd->text_len, stack);
    cairo_text_extents_t extents;
    double measured_x, measured_y, allowed_x, allowed_y;
    double measured, allowed;
    if (!text) return 0;
    cairo_text_extents(cr, text, &extents);
    *x_advance = extents.x_advance;
    measured_x = extents.x_advance;
    measured_y = 0.0;
    allowed_x = cmd->width;
    allowed_y = 0.0;
    cairo_user_to_device_distance(cr, &measured_x, &measured_y);
    cairo_user_to_device_distance(cr, &allowed_x, &allowed_y);
    measured = hypot(measured_x, measured_y);
    allowed = hypot(allowed_x, allowed_y);
    if (text != stack) free(text);
    return measured <= allowed + 1.0;
}

static int cclover_cairo_scene_conforms(cairo_t *cr, const CcloverScene *scene,
                                        CcloverCairoDrawMode mode,
                                        int cull_to_redraw_mask,
                                        CcloverTextAdvance *text_advances) {
    int redraw_mask_valid = cclover_redraw_mask_valid(scene, cull_to_redraw_mask);
    uint32_t font_size = 0;
    uint32_t font_bold = 0;
    int font_valid = 0;
    size_t i;
    for (i = 0; i < scene->command_count; ++i) {
        const CcloverCommand *cmd = &scene->commands[i];
        uint32_t bold;
        double x_advance;
        if (!cclover_draw_command(scene, mode, i, cull_to_redraw_mask,
                                  redraw_mask_valid) ||
            cmd->kind != CCLOVER_CMD_TEXT ||
            !(cmd->flags & CCLOVER_TEXT_MUST_FIT))
            continue;
        bold = (cmd->flags & CCLOVER_TEXT_BOLD) != 0;
        cclover_ensure_font_selected(cr, cmd->text_size, bold,
                                     &font_size, &font_bold, &font_valid);
        if (!cclover_cairo_text_fits(cr, cmd, &x_advance)) return 0;
        if (text_advances && (cmd->flags & CCLOVER_TEXT_END)) {
            text_advances[i].x_advance = x_advance;
            text_advances[i].valid = 1;
        }
    }
    return 1;
}

static CcloverTextAdvance *cclover_cairo_prepare_text_advances(
    CcloverCairoRenderer *renderer, const CcloverScene *scene) {
    CcloverTextAdvance *resized;
    renderer->prepared_commands = NULL;
    renderer->prepared_command_count = 0;
    if (scene->command_count == 0) return NULL;
    if (renderer->text_advance_capacity < scene->command_count) {
        resized = realloc(renderer->text_advances,
                          scene->command_count * sizeof(*renderer->text_advances));
        if (!resized) return NULL;
        renderer->text_advances = resized;
        renderer->text_advance_capacity = scene->command_count;
    }
    memset(renderer->text_advances, 0,
           scene->command_count * sizeof(*renderer->text_advances));
    return renderer->text_advances;
}

void cclover_cairo_renderer_destroy(CcloverCairoRenderer *renderer) {
    if (!renderer) return;
    free(renderer->text_advances);
    renderer->text_advances = NULL;
    renderer->text_advance_capacity = 0;
    renderer->prepared_commands = NULL;
    renderer->prepared_command_count = 0;
}

int cclover_cairo_scene_fits(CcloverCairoRenderer *renderer, cairo_t *cr,
                             const CcloverScene *scene,
                             CcloverCairoDrawMode mode,
                             int cull_to_redraw_mask) {
    CcloverTextAdvance *text_advances =
        cclover_cairo_prepare_text_advances(renderer, scene);
    if (!cclover_cairo_scene_conforms(cr, scene, mode, cull_to_redraw_mask,
                                      text_advances)) {
        if (!renderer->typography_violation_reported) {
            fprintf(stderr,
                    "cclover-mon: Cairo text exceeded its authoritative Scene slot\n");
            renderer->typography_violation_reported = 1;
        }
        return 0;
    }
    if (text_advances) {
        renderer->prepared_commands = scene->commands;
        renderer->prepared_command_count = scene->command_count;
    }
    renderer->typography_violation_reported = 0;
    return 1;
}

int cclover_cairo_validate_scene(cairo_t *cr, const CcloverScene *scene,
                                 CcloverCairoDrawMode mode,
                                 int cull_to_redraw_mask) {
    return cclover_cairo_scene_conforms(cr, scene, mode, cull_to_redraw_mask, NULL);
}

void cclover_cairo_execute_validated_scene(CcloverCairoRenderer *renderer, cairo_t *cr,
                                           const CcloverScene *scene, int clear,
                                           CcloverCairoDrawMode mode,
                                           int cull_to_redraw_mask) {
    size_t i, j;
    uint32_t font_size = 0;
    uint32_t font_bold = 0;
    int font_valid = 0;
    int redraw_mask_valid = cclover_redraw_mask_valid(scene, cull_to_redraw_mask);
    int prepared = renderer && renderer->prepared_commands == scene->commands &&
                   renderer->prepared_command_count == scene->command_count;
    cairo_save(cr);
    if (clear) {
        cairo_set_operator(cr, CAIRO_OPERATOR_SOURCE);
        cairo_set_source_rgba(cr, 0, 0, 0, 0);
        cairo_paint(cr);
    }
    cairo_set_operator(cr, CAIRO_OPERATOR_OVER);

    for (i = 0; i < scene->command_count; ++i) {
        const CcloverCommand *cmd = &scene->commands[i];
        if (!cclover_draw_command(scene, mode, i, cull_to_redraw_mask,
                                  redraw_mask_valid)) continue;
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
            const CcloverTextAdvance *advance =
                prepared ? &renderer->text_advances[i] : NULL;
            cclover_draw_text(renderer, cr, cmd, advance,
                              &font_size, &font_bold, &font_valid);
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
    if (renderer) {
        renderer->prepared_commands = NULL;
        renderer->prepared_command_count = 0;
    }
}

int cclover_cairo_draw_scene(CcloverCairoRenderer *renderer, cairo_t *cr,
                             const CcloverScene *scene, int clear,
                             CcloverCairoDrawMode mode) {
    if (!cclover_cairo_scene_fits(renderer, cr, scene, mode, 0)) return 1;
    cclover_cairo_execute_validated_scene(renderer, cr, scene, clear, mode, 0);
    return 0;
}
