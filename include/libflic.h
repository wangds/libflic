#ifndef LIBFLIC_H
#define LIBFLIC_H

#include <stddef.h>

struct FlicFile;
struct CRasterMut;

enum {
    FLICRS_SUCCESS = 0,
    FLICRS_ERROR = 1,

    // flicrs_read_next_frame
    FLICRS_ENDED = 2,
    FLICRS_LOOPED = 4,
    FLICRS_PALETTE_UPDATED = 8,
};

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

extern unsigned int
flicrs_decode_fli_brun(
        const unsigned char *src, size_t src_len,
        struct CRasterMut *dst);

extern unsigned int
flicrs_decode_fli_copy(
        const unsigned char *src, size_t src_len,
        struct CRasterMut *dst);

/*--------------------------------------------------------------*/
/* FLIC                                                         */
/*--------------------------------------------------------------*/

extern struct FlicFile *
flicrs_open(
        const char *filename);

extern void
flicrs_close(
        struct FlicFile *flic);

extern unsigned int
flicrs_frame(
        struct FlicFile *flic);

extern unsigned int
flicrs_frame_count(
        struct FlicFile *flic);

extern unsigned int
flicrs_width(
        struct FlicFile *flic);

extern unsigned int
flicrs_height(
        struct FlicFile *flic);

extern unsigned int
flicrs_speed_jiffies(
        struct FlicFile *flic);

extern unsigned int
flicrs_read_next_frame(
        struct FlicFile *flic, struct CRasterMut *raster);

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
