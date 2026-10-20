#ifndef CCLOVER_WINDOWS_ALPHA_POLICY_H
#define CCLOVER_WINDOWS_ALPHA_POLICY_H

#include <stddef.h>
#include <stdint.h>

/*
 * GDI does not maintain a destination alpha channel. The Windows host renders the
 * same Scene twice, once over opaque black and once over opaque white, and
 * recovers per-pixel coverage from the difference:
 *
 *   over_black = premultiplied colour
 *   over_white = premultiplied colour + (1 - alpha) * 255
 *
 * Pixels are 0x00RRGGBB; the top byte of both inputs is ignored.
 * The result is premultiplied 0xAARRGGBB, as UpdateLayeredWindow requires.
 */
uint32_t cclover_alpha_recover_pixel(uint32_t over_black, uint32_t over_white);

/* Replaces every pixel in `black` with its recovered premultiplied ARGB value. */
void cclover_alpha_recover(uint32_t *black, const uint32_t *white, size_t count);

#endif
