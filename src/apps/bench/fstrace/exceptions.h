// vim:ft=cpp
/*
 * (c) 2007-2013 Carsten Weinhold <weinhold@os.inf.tu-dresden.de>
 *     economic rights: Technische Universität Dresden (Germany)
 *
 * This file is part of TUD:OS, which is distributed under the terms of the
 * GNU General Public License 2. Please see the COPYING-GPL-2 file for details.
 */

#pragma once

#if defined(__LINUX__)
#   include <string>
#   include <sstream>
#   define str_t                        std::string
#   define sstream_t                    std::ostringstream
#else
#   include <base/util/String.h>
#   include <m3/stream/Standard.h>
#   define str_t                        m3::String
#   define sstream_t                    m3::OStringStream
#endif


class Exception {

  public:
    virtual ~Exception() { };

    virtual str_t &msg() {

        return text;
    };

  protected:
    str_t text;
};


class NotSupportedException: public Exception {

  public:
    NotSupportedException(int lineNo) {

        sstream_t s;
        s << "Not supported in line #" << lineNo;
        text = s.str();
    };
};


class OutOfMemoryException: public Exception {

  public:
    OutOfMemoryException() {

        text = "Out of memory";
    };
};


class ReturnValueException: public Exception {

  public:
    ReturnValueException(int got, int expected, int lineNo = -1) {

        sstream_t s;
        s << "Unexpected return value " << got << " instead of " << expected;
        if (lineNo >= 0)
            s << " in line #" << lineNo;
        text = s.str();
    };
};


class ParseException: public Exception {

  public:
    ParseException(const str_t &line, int lineNo = -1, int colNo = -1) {

        sstream_t s;
        s << "Parse error";
        if (lineNo >= 0)
            s << " in line " << lineNo;
        if (colNo >= 0)
            s << " at col " << colNo;
        s << ": " << line.c_str();
        text = s.str();
    }
};


class IoException: public Exception {

  public:
    IoException(const str_t &msg, const str_t &name = "", int errorNo = 0) {

        sstream_t s;
        s << "I/O error ";
        if (errorNo != 0)
           s << errorNo;
        if ( name.length() > 0)
            s << " for file " << name.c_str() << ": " << msg.c_str();
        text = s.str();
    }
};
