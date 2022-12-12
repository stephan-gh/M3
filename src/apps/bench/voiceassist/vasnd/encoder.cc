/*
 * Copyright (C) 2021, Tendsin Mende <tendsin.mende@mailbox.tu-dresden.de>
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

#include "FLAC++/encoder.h"

#include <cstring>
#include <stdexcept>

#include "FLAC++/metadata.h"
#include "encoder.h"

class OurEncoder : public FLAC::Encoder::Stream {
public:
    uint32_t total_samples = 0; /* can use a 32-bit number due to WAVE size limitations */

    OurEncoder(void *outbuf, size_t outmax)
        : FLAC::Encoder::Stream(),
          outbuf(outbuf),
          outmax(outmax),
          outpos(0) {
    }

    virtual ::FLAC__StreamEncoderWriteStatus write_callback(const FLAC__byte buffer[], size_t bytes,
                                                            uint32_t, uint32_t) {
        if(outpos + bytes > outmax)
            throw std::runtime_error("Output buffer exhaused");

        memcpy(static_cast<char *>(outbuf) + outpos, buffer, bytes);
        outpos += bytes;
        return ::FLAC__STREAM_ENCODER_WRITE_STATUS_OK;
    }

    virtual void progress_callback(FLAC__uint64 bytes_written, FLAC__uint64 samples_written,
                                   uint32_t frames_written, uint32_t total_frames_estimate) {
        fprintf(stderr, "wrote %lu bytes, %lu/%u samples, %u/%u frames\n", bytes_written,
                samples_written, total_samples, frames_written, total_frames_estimate);
    }

    void *outbuf;
    size_t outmax;
    size_t outpos;
};

static constexpr size_t READSIZE = 1024;

static FLAC__int32 pcm[READSIZE /*samples*/ * 2 /*channels*/];

size_t encode(const uint8_t *indata, size_t inlen, void *outbuf, size_t outmax) {
    bool ok = true;
    OurEncoder encoder(outbuf, outmax);
    FLAC__StreamEncoderInitStatus init_status;
    FLAC__StreamMetadata *metadata[2];
    FLAC__StreamMetadata_VorbisComment_Entry entry;
    uint32_t sample_rate = 0;
    uint32_t channels = 0;
    uint32_t bps = 0;

    if(memcmp(indata, "RIFF", 4) ||
       memcmp(indata + 8, "WAVEfmt \020\000\000\000\001\000\002\000", 16) ||
       memcmp(indata + 32, "\004\000\020\000data", 8)) {
        fprintf(
            stderr,
            "ERROR: invalid/unsupported WAVE file, only 16bps stereo WAVE in canonical form allowed\n");
        return 0;
    }

    sample_rate = ((((((uint32_t)indata[27] << 8) | indata[26]) << 8) | indata[25]) << 8) |
                  indata[24];
    channels = 2;
    bps = 16;
    encoder.total_samples =
        (((((((uint32_t)indata[43] << 8) | indata[42]) << 8) | indata[41]) << 8) | indata[40]) / 4;

    /* check the encoder */
    if(!encoder) {
        fprintf(stderr, "ERROR: allocating encoder\n");
        return 0;
    }

    ok &= encoder.set_verify(true);
    ok &= encoder.set_compression_level(5);
    ok &= encoder.set_channels(channels);
    ok &= encoder.set_bits_per_sample(bps);
    ok &= encoder.set_sample_rate(sample_rate);
    ok &= encoder.set_total_samples_estimate(encoder.total_samples);

    /* now add some metadata; we'll add some tags and a padding block */
    if(ok) {
        if((metadata[0] = FLAC__metadata_object_new(FLAC__METADATA_TYPE_VORBIS_COMMENT)) == NULL ||
           (metadata[1] = FLAC__metadata_object_new(FLAC__METADATA_TYPE_PADDING)) == NULL ||
           /* there are many tag (vorbiscomment) functions but these are convenient for this
              particular use: */
           !FLAC__metadata_object_vorbiscomment_entry_from_name_value_pair(&entry, "ARTIST",
                                                                           "Some Artist") ||
           !FLAC__metadata_object_vorbiscomment_append_comment(metadata[0], entry,
                                                               /*copy=*/false) || /* copy=false: let
                                                                                     metadata object
                                                                                     take control of
                                                                                     entry's
                                                                                     allocated
                                                                                     string */
           !FLAC__metadata_object_vorbiscomment_entry_from_name_value_pair(&entry, "YEAR",
                                                                           "1984") ||
           !FLAC__metadata_object_vorbiscomment_append_comment(metadata[0], entry,
                                                               /*copy=*/false)) {
            fprintf(stderr, "ERROR: out of memory or tag error\n");
            ok = false;
        }
        else {
            metadata[1]->length = 1234; /* set the padding length */

            ok = encoder.set_metadata(metadata, 2);
        }
    }

    /* initialize encoder */
    if(ok) {
        init_status = encoder.init();
        if(init_status != FLAC__STREAM_ENCODER_INIT_STATUS_OK) {
            fprintf(stderr, "ERROR: initializing encoder: %s\n",
                    FLAC__StreamEncoderInitStatusString[init_status]);
            ok = false;
        }
    }

    /* read blocks of samples from WAVE file and feed to encoder */
    size_t pos = 44;
    if(ok) {
        size_t left = (size_t)encoder.total_samples;
        while(ok && left && pos < inlen) {
            size_t need = left > READSIZE ? READSIZE : left;
            /* convert the packed little-endian 16-bit PCM samples from WAVE into an interleaved
             * FLAC__int32 buffer for libFLAC */
            size_t i;
            for(i = 0; i < need * channels; i++) {
                /* inefficient but simple and works on big- or little-endian machines */
                pcm[i] = (FLAC__int32)(((FLAC__int16)(FLAC__int8)indata[pos + 2 * i + 1] << 8) |
                                       (FLAC__int16)indata[pos + 2 * i]);
            }
            /* feed samples to encoder */
            ok = encoder.process_interleaved(pcm, need);
            left -= need;
            pos += channels * (bps / 8) * need;
        }
    }

    ok &= encoder.finish();

    /* now that encoding is finished, the metadata can be freed */
    FLAC__metadata_object_delete(metadata[0]);
    FLAC__metadata_object_delete(metadata[1]);

    return encoder.outpos;
}
