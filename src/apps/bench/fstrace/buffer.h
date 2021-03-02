// vim:ft=cpp
/*
 * (c) 2007-2013 Carsten Weinhold <weinhold@os.inf.tu-dresden.de>
 *     economic rights: Technische Universität Dresden (Germany)
 *
 * This file is part of TUD:OS, which is distributed under the terms of the
 * GNU General Public License 2. Please see the COPYING-GPL-2 file for details.
 */

#pragma once

#include "exceptions.h"

class Buffer {
  public:
    static constexpr size_t MaxBufferSize = 8*1024;

    Buffer(size_t maxReadSize = MaxBufferSize,
           size_t maxWriteSize = MaxBufferSize);
    virtual ~Buffer();

    char *readBuffer(size_t size);
    char *writeBuffer(size_t size);

  protected:
    size_t maxReadSize;
    size_t maxWriteSize;
    char  *readBuf;
    char  *readBufAligned;
    char  *writeBuf;
    char  *writeBufAligned;
};
