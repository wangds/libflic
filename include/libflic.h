#ifndef LIBFLIC_H
#define LIBFLIC_H

#include <stddef.h>

struct FlicFile;
struct CRaster;
struct CRasterMut;

enum {
    FLICRS_SUCCESS = 0,
    FLICRS_ERROR = 1,

    // flicrs_encode
    FLICRS_BUFFER_TOO_SMALL = 2,

    // flicrs_read_next_frame
    FLICRS_ENDED = 2,
    FLICRS_LOOPED = 4,
    FLICRS_PALETTE_UPDATED = 8,
};

/*--------------------------------------------------------------*/
/* Codecs                                                       */
/*--------------------------------------------------------------*/

extern unsigned int
flicrs_decode_fli_wrun(
        const unsigned char *src, size_t src_len,
        struct CRasterMut *dst);

extern unsigned int
flicrs_decode_fli_color256(
        const unsigned char *src, size_t src_len,
        struct CRasterMut *dst);

extern unsigned int
flicrs_decode_fli_ss2(
        const unsigned char *src, size_t src_len,
        struct CRasterMut *dst);

extern unsigned int
flicrs_decode_fli_sbsrsc(
        const unsigned char *src, size_t src_len,
        struct CRasterMut *dst);

extern unsigned int
flicrs_decode_fli_color64(
        const unsigned char *src, size_t src_len,
        struct CRasterMut *dst);

extern unsigned int
flicrs_decode_fli_lc(
        const unsigned char *src, size_t src_len,
        struct CRasterMut *dst);

extern unsigned int
flicrs_decode_fli_black(
        struct CRasterMut *dst);

extern unsigned int
flicrs_decode_fli_icolors(
        struct CRasterMut *dst);

extern unsigned int
flicrs_decode_fli_brun(
        const unsigned char *src, size_t src_len,
        struct CRasterMut *dst);

extern unsigned int
flicrs_decode_fli_copy(
        const unsigned char *src, size_t src_len,
        struct CRasterMut *dst);

extern unsigned int
flicrs_decode_fps_brun(
        const unsigned char *src, size_t src_len, size_t src_w, size_t src_h,
        struct CRasterMut *dst);

extern unsigned int
flicrs_encode_fli_color64(
        struct CRaster *opt_prev, struct CRaster *next,
        unsigned char *out_buf, size_t max_len, size_t *out_len);

extern unsigned int
flicrs_encode_fli_lc(
        struct CRaster *prev, struct CRaster *next,
        unsigned char *out_buf, size_t max_len, size_t *out_len);

extern unsigned int
flicrs_encode_fli_brun(
        struct CRaster *next,
        unsigned char *out_buf, size_t max_len, size_t *out_len);

extern unsigned int
flicrs_encode_fli_copy(
        struct CRaster *next,
        unsigned char *out_buf, size_t max_len, size_t *out_len);

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
flicrs_speed_msec(
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

extern struct CRaster *
flicrs_raster_alloc(
        size_t x, size_t y, size_t w, size_t h, size_t stride,
        const unsigned char *buf, size_t buf_len,
        const unsigned char *pal, size_t pal_len);

extern struct CRasterMut *
flicrs_raster_mut_alloc(
        size_t x, size_t y, size_t w, size_t h, size_t stride,
        unsigned char *buf, size_t buf_len,
        unsigned char *pal, size_t pal_len);

extern void
flicrs_raster_free(struct CRaster *raster);

extern void
flicrs_raster_mut_free(struct CRasterMut *raster);

#endif
