#ifndef LIBFLIC_H
#define LIBFLIC_H

#include <stddef.h>

struct CRasterMut;

/*--------------------------------------------------------------*/
/* Codecs                                                       */
/*--------------------------------------------------------------*/

extern unsigned int
flicrs_decode_fli_color64(
        const unsigned char *src, size_t src_len,
        struct CRasterMut *dst);

extern unsigned int
flicrs_decode_fli_lc(
        const unsigned char *src, size_t src_len,
        struct CRasterMut *dst);

extern void
flicrs_decode_fli_black(
        struct CRasterMut *dst);

/*--------------------------------------------------------------*/
/* Raster                                                       */
/*--------------------------------------------------------------*/

extern struct CRasterMut *
flicrs_raster_mut_alloc(
        size_t x, size_t y, size_t w, size_t h, size_t stride,
        unsigned char *buf, size_t buf_len,
        unsigned char *pal, size_t pal_len);

extern void
flicrs_raster_mut_free(struct CRasterMut *raster);

#endif
