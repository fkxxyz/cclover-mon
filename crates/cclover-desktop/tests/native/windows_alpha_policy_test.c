#include <assert.h>

#include "alpha_policy.h"

#define PIXEL(r, g, b) (((uint32_t)(r) << 16) | ((uint32_t)(g) << 8) | (uint32_t)(b))

static uint32_t premultiplied_over(uint32_t value, uint32_t alpha) {
    return (value * alpha + 127u) / 255u;
}

int main(void) {
    /* Opaque content is identical over both bases. */
    assert(cclover_alpha_recover_pixel(PIXEL(0x12, 0x34, 0x56), PIXEL(0x12, 0x34, 0x56)) ==
           (0xff000000u | PIXEL(0x12, 0x34, 0x56)));
    assert(cclover_alpha_recover_pixel(0, PIXEL(255, 255, 255)) == 0);

    /* The top byte written by GDI is not part of the colour. */
    assert(cclover_alpha_recover_pixel(0xff000000u | PIXEL(1, 2, 3),
                                       0x7f000000u | PIXEL(1, 2, 3)) ==
           (0xff000000u | PIXEL(1, 2, 3)));

    /* The product's panel background: #0b1020 at alpha 217 (0.85). */
    {
        const uint32_t alpha = 217u;
        uint32_t r = premultiplied_over(0x0b, alpha);
        uint32_t g = premultiplied_over(0x10, alpha);
        uint32_t b = premultiplied_over(0x20, alpha);
        uint32_t white = 255u - alpha;
        uint32_t recovered = cclover_alpha_recover_pixel(
            PIXEL(r, g, b), PIXEL(r + white, g + white, b + white));
        assert((recovered >> 24) == alpha);
        assert(((recovered >> 16) & 0xffu) == r);
        assert(((recovered >> 8) & 0xffu) == g);
        assert((recovered & 0xffu) == b);
    }

    /* A card at alpha 204 (0.80) is recovered as the same coverage. */
    {
        const uint32_t alpha = 204u;
        uint32_t r = premultiplied_over(0x15, alpha);
        uint32_t g = premultiplied_over(0x1b, alpha);
        uint32_t b = premultiplied_over(0x2d, alpha);
        uint32_t white = 255u - alpha;
        uint32_t recovered = cclover_alpha_recover_pixel(
            PIXEL(r, g, b), PIXEL(r + white, g + white, b + white));
        assert((recovered >> 24) == alpha);
    }

    /* Rounding noise may not make a premultiplied channel exceed alpha. */
    {
        uint32_t recovered = cclover_alpha_recover_pixel(PIXEL(200, 0, 0), PIXEL(255, 55, 55));
        assert((recovered >> 24) == 200u);
        assert(((recovered >> 16) & 0xffu) <= (recovered >> 24));
    }

    /* Buffer recovery applies the pixel rule in place. */
    {
        uint32_t black[3] = { 0, PIXEL(9, 9, 9), PIXEL(10, 20, 30) };
        const uint32_t white[3] = { PIXEL(255, 255, 255), PIXEL(9, 9, 9), PIXEL(10, 20, 30) };
        cclover_alpha_recover(black, white, 3);
        assert(black[0] == 0);
        assert(black[1] == (0xff000000u | PIXEL(9, 9, 9)));
        assert(black[2] == (0xff000000u | PIXEL(10, 20, 30)));
    }
    return 0;
}
