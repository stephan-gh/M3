/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#pragma once

#include <base/Common.h>
#include <base/stream/Format.h>

#include <string>
#include <vector>

#include "base/Compiler.h"

enum TokenType {
    PIPE,
    LESS_THAN,
    GREATER_THAN,
    DOLLAR,
    ASSIGN,
    STRING,
};

static inline m3::OStream &operator<<(m3::OStream &os, TokenType type) {
    switch(type) {
        case PIPE: os.write_string("'|'"); break;
        case LESS_THAN: os.write_string("'<'"); break;
        case GREATER_THAN: os.write_string("'>'"); break;
        case DOLLAR: os.write_string("'$'"); break;
        case ASSIGN: os.write_string("'='"); break;
        case STRING: os.write_string("T_STRING"); break;
    }
    return os;
}

struct Token {
    static Token from_char(char c) {
        switch(c) {
            case '|': return Token(TokenType::PIPE, c);
            case '<': return Token(TokenType::LESS_THAN, c);
            case '>': return Token(TokenType::GREATER_THAN, c);
            case '$': return Token(TokenType::DOLLAR, c);
            case '=': return Token(TokenType::ASSIGN, c);
            default: UNREACHED;
        }
    }

    explicit Token(TokenType type, char c) : _type(type), _str(1, c) {
    }
    explicit Token(const char *s, size_t len) : _type(TokenType::STRING), _str(s, len) {
    }

    TokenType type() const {
        return _type;
    }
    char simple() const {
        return _str[0];
    }
    const std::string &string() const {
        return _str;
    }

    friend m3::OStream &operator<<(m3::OStream &os, const Token &t) {
        using namespace m3;
        if(t.type() == TokenType::STRING)
            format_to(os, "'{}'"_cf, t.string());
        else
            format_to(os, "'{}'"_cf, t.simple());
        return os;
    }

private:
    TokenType _type;
    std::string _str;
};

class Tokenizer {
    Tokenizer() = delete;

public:
    static std::vector<Token> tokenize(const char *input);
};
