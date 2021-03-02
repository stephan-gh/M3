// vim:ft=cpp
/*
 * (c) 2007-2013 Carsten Weinhold <weinhold@os.inf.tu-dresden.de>
 *     economic rights: Technische Universität Dresden (Germany)
 *
 * This file is part of TUD:OS, which is distributed under the terms of the
 * GNU General Public License 2. Please see the COPYING-GPL-2 file for details.
 */

#include "buffer.h"


Buffer::Buffer(size_t maxReadSize, size_t maxWriteSize) {

    this->maxReadSize  = maxReadSize;
    this->maxWriteSize = maxWriteSize;

    readBuf  = new char[maxReadSize + PAGE_SIZE];
    writeBuf = new char[maxWriteSize + PAGE_SIZE];

    if (readBuf == 0 || writeBuf == 0) {
        delete [] readBuf;
        delete [] writeBuf;
        throw OutOfMemoryException();
    }

    readBufAligned = reinterpret_cast<char*>(
        m3::Math::round_up(reinterpret_cast<uintptr_t>(readBuf), PAGE_SIZE));
    writeBufAligned = reinterpret_cast<char*>(
        m3::Math::round_up(reinterpret_cast<uintptr_t>(writeBuf), PAGE_SIZE));
}


Buffer::~Buffer() {

    delete [] readBuf;
    delete [] writeBuf;
}


char *Buffer::readBuffer(size_t size) {

    if (size > maxReadSize)
        throw OutOfMemoryException();

    return readBufAligned;
}


char *Buffer::writeBuffer(size_t size) {

    if (size > maxWriteSize)
        throw OutOfMemoryException();

    return writeBufAligned;
}
