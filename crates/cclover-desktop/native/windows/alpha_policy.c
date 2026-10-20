#include "alpha_policy.h"

static uint32_t channel(uint32_t pixel, unsigned shift) {
    return (pixel >> shift) & 0xffu;
}

uint32_t cclover_alpha_recover_pixel(uint32_t over_black, uint32_t over_white) {
    uint32_t difference = 0;
    uint32_t alpha, result;
    unsigned shift;

    for (shift = 0; shift <= 16; shift += 8) {
        uint32_t black = channel(over_black, shift);
        uint32_t white = channel(over_white, shift);
        difference += white > black ? white - black : 0;
    }
    difference = (difference + 1u) / 3u;
    alpha = difference >= 255u ? 0u : 255u - difference;
    if (alpha == 0) return 0;

    result = alpha << 24;
    for (shift = 0; shift <= 16; shift += 8) {
        uint32_t value = channel(over_black, shift);
        if (value > alpha) value = alpha;
        result |= value << shift;
    }
    return result;
}

void cclover_alpha_recover(uint32_t *black, const uint32_t *white, size_t count) {
    size_t i;
    for (i = 0; i < count; ++i) {
        black[i] = cclover_alpha_recover_pixel(black[i], white[i]);
    }
}
