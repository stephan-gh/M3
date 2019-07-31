// vim:ft=cpp
/*
 * (c) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * (c) 2007-2013 Carsten Weinhold <weinhold@os.inf.tu-dresden.de>
 *     economic rights: Technische Universität Dresden (Germany)
 *
 * This file is part of TUD:OS, which is distributed under the terms of the
 * GNU General Public License 2. Please see the COPYING-GPL-2 file for details.
 */

#pragma once

#include <m3/session/LoadGen.h>

#include "buffer.h"
#include "op_types.h"
#include "traces.h"

class TracePlayer {
  public:
    typedef enum { File, Dir } File_type;

    TracePlayer(char const *rootDir)
        : pathPrefix(rootDir) { }

    virtual ~TracePlayer() { };

    virtual int play(Trace *trace, m3::LoadGen::Channel *chan,
                     bool data = true, bool stdio = false,
                     bool keep_time = false, bool verbose = false);

  protected:
    const char *pathPrefix;
};
