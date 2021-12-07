/*
 * Copyright (C) 2015-2016, Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * This file is part of M3 (Microkernel-based SysteM for Heterogeneous Manycores).
 *
 * M3 is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License version 2 as
 * published by the Free Software Foundation.
 *
 * M3 is distributed in the hope that it will be useful, but
 * WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
 * General Public License version 2 for more details.
 */

#include <base/time/Profile.h>

#include <m3/stream/Standard.h>

#include <jpeglib.h>

#include <string.h>
#include <stdio.h>

using namespace m3;

static void write_JPEG_file(const char *filename, int quality,
                            unsigned int width, unsigned int height) {
    /* This struct contains the JPEG compression parameters and pointers to
     * working space (which is allocated as needed by the JPEG library).
     * It is possible to have several such structures, representing multiple
     * compression/decompression processes, in existence at once.  We refer
     * to any one struct (and its associated working data) as a "JPEG object".
     */
    struct jpeg_compress_struct cinfo;
    /* This struct represents a JPEG error handler.  It is declared separately
     * because applications often want to supply a specialized error handler
     * (see the second half of this file for an example).  But here we just
     * take the easy way out and use the standard error handler, which will
     * print a message on stderr and call exit() if compression fails.
     * Note that this struct must live as long as the main JPEG parameter
     * struct, to avoid dangling-pointer problems.
     */
    struct jpeg_error_mgr jerr;
    /* More stuff */
    FILE * outfile;       /* target file */
    JSAMPROW row_pointer[1];  /* pointer to JSAMPLE row[s] */
    // unsigned int row_stride;       /* physical row width in image buffer */

    /* Step 1: allocate and initialize JPEG compression object */

    /* We have to set up the error handler first, in case the initialization
     * step fails.  (Unlikely, but it could happen if you are out of memory.)
     * This routine fills in the contents of struct jerr, and returns jerr's
     * address which we place into the link field in cinfo.
     */
    cinfo.err = jpeg_std_error(&jerr);
    /* Now we can initialize the JPEG compression object. */
    jpeg_create_compress(&cinfo);

    /* Step 2: specify data destination (eg, a file) */
    /* Note: steps 2 and 3 can be done in either order. */

    /* Here we use the library-supplied code to send compressed data to a
     * stdio stream.  You can also write your own code to do something else.
     * VERY IMPORTANT: use "b" option to fopen() if you are on a machine that
     * requires it in order to write binary files.
     */
    if ((outfile = fopen(filename, "wb")) == NULL) {
        fprintf(stderr, "can't open %s\n", filename);
        exit(1);
    }
    jpeg_stdio_dest(&cinfo, outfile);

    /* Step 3: set parameters for compression */

    /* First we supply a description of the input image.
     * Four fields of the cinfo struct must be filled in:
     */
    cinfo.image_width = width;  /* image width and height, in pixels */
    cinfo.image_height = height;
    cinfo.input_components = 3;       /* # of color components per pixel */
    cinfo.in_color_space = JCS_RGB;   /* colorspace of input image */
    /* Now use the library's routine to set default compression parameters.
     * (You must set at least cinfo.in_color_space before calling this,
     * since the defaults depend on the source color space.)
     */
    jpeg_set_defaults(&cinfo);
    /* Now you can set any non-default parameters you wish to.
     * Here we just illustrate the use of quality (quantization table) scaling:
     */
    jpeg_set_quality(&cinfo, quality, TRUE /* limit to baseline-JPEG values */);

    /* Step 4: Start compressor */

    /* TRUE ensures that we will write a complete interchange-JPEG file.
     * Pass TRUE unless you are very sure of what you're doing.
     */
    jpeg_start_compress(&cinfo, TRUE);

    /* Step 5: while (scan lines remain to be written) */
    /*           jpeg_write_scanlines(...); */

    /* Here we use the library's state variable cinfo.next_scanline as the
     * loop counter, so that we don't have to keep track ourselves.
     * To keep things simple, we pass one scanline per call; you can pass
     * more if you wish, though.
     */
    // row_stride = image_width * 3; /* JSAMPLEs per row in image_buffer */


    unsigned char *raw_row = new unsigned char[width * 3];
    memset(raw_row, 0, width * 3);

    while (cinfo.next_scanline < cinfo.image_height) {
        /* jpeg_write_scanlines expects an array of pointers to scanlines.
        * Here the array is only one element long, but you could pass
        * more than one scanline at a time if that's more convenient.
        */
        // row_pointer[0] = & image_buffer[cinfo.next_scanline * row_stride];
        row_pointer[0] = raw_row;
        (void) jpeg_write_scanlines(&cinfo, row_pointer, 1);
    }

    /* Step 6: Finish compression */

    jpeg_finish_compress(&cinfo);
    /* After finish_compress, we can close the output file. */
    fclose(outfile);

    /* Step 7: release JPEG compression object */

    /* This is an important step since it will release a good deal of memory. */
    jpeg_destroy_compress(&cinfo);

    /* And we're done! */
}

int main() {
    int quali[] = {50, 75, 100};
    size_t sizes[] = {500, 1000, 2000};
    for(size_t s = 0; s < ARRAY_SIZE(sizes); ++s) {
        for(size_t q = 0; q < ARRAY_SIZE(quali); ++q) {
            Profile pr(2, 1);
            auto res = pr.run<CycleInstant>([quali, sizes, q, s] {
                write_JPEG_file("/myjpeg.jpeg", quali[q], sizes[s], sizes[s]);
            });
            cout << "JPEG creation"
                 << " (quali=" << quali[q]
                 << ", size=" << ((sizes[s] * sizes[s] * 4) / 1024) << " KiB): " << res << "\n";
        }

        uint32_t *src = new uint32_t[sizes[s] * sizes[s]];
        uint32_t *dst = new uint32_t[sizes[s] * sizes[s]];

        Profile pr(2, 1);
        auto res = pr.run<CycleInstant>([src, dst, sizes, s] {
            memcpy(dst, src, sizes[s] * sizes[s] * sizeof(uint32_t));
        });
        cout << "memcpy"
             << " (size=" << ((sizes[s] * sizes[s] * 4) / 1024) << " KiB): " << res << "\n";

        delete[] src;
        delete[] dst;
    }
    return 0;
}
