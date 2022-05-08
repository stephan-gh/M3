// vim:ft=cpp
/*
 * (c) 2007-2013 Carsten Weinhold <weinhold@os.inf.tu-dresden.de>
 *     economic rights: Technische Universität Dresden (Germany)
 *
 * This file is part of TUD:OS, which is distributed under the terms of the
 * GNU General Public License 2. Please see the COPYING-GPL-2 file for details.
 */

#pragma once

#include <sstream>
#include <string>

class Exception {
public:
    virtual ~Exception(){};

    virtual std::string &msg() {
        return text;
    };

protected:
    std::string text;
};

class NotSupportedException : public Exception {
public:
    NotSupportedException(int lineNo) {
        std::ostringstream s;
        s << "Not supported in line #" << lineNo;
        text = s.str();
    };
};

class OutOfMemoryException : public Exception {
public:
    OutOfMemoryException() {
        text = "Out of memory";
    };
};

class ReturnValueException : public Exception {
public:
    ReturnValueException(int got, int expected, int lineNo = -1) {
        std::ostringstream s;
        s << "Unexpected return value " << got << " instead of " << expected;
        if(lineNo >= 0)
            s << " in line #" << lineNo;
        text = s.str();
    };
};

class ParseException : public Exception {
public:
    ParseException(const std::string &line, int lineNo = -1, int colNo = -1) {
        std::ostringstream s;
        s << "Parse error";
        if(lineNo >= 0)
            s << " in line " << lineNo;
        if(colNo >= 0)
            s << " at col " << colNo;
        s << ": " << line.c_str();
        text = s.str();
    }
};

class IoException : public Exception {
public:
    IoException(const std::string &msg, const std::string &name = "", int errorNo = 0) {
        std::ostringstream s;
        s << "I/O error ";
        if(errorNo != 0)
            s << errorNo;
        if(name.length() > 0)
            s << " for file " << name.c_str() << ": " << msg.c_str();
        text = s.str();
    }
};
