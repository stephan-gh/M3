/*
 * Copyright (C) 2016-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2021 Nils Asmussen, Barkhausen Institut
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

#include "Tokenizer.h"

#include <string.h>

enum State {
    DEFAULT,
    IN_STR,
};

std::vector<Token> Tokenizer::tokenize(const char *input) {
    std::vector<Token> tokens;

    State state = DEFAULT;
    bool seen_eq = false;
    bool in_vars = true;

    size_t start = 0;

    size_t len = strlen(input);
    for(size_t i = 0; i < len; ++i) {
        char c = input[i];

        switch(state) {
            case DEFAULT:
                switch(c) {
                    case '\n': i = len; break;

                    case ' ':
                    case '\t':
                        if(i > start) {
                            // if we haven't seen "=", this is not a variable assignment and
                            // therefore we have left the variable part
                            if(!seen_eq)
                                in_vars = false;
                            seen_eq = false;
                            tokens.push_back(Token(input + start, i - start));
                            start = i + 1;
                        }
                        else
                            start++;
                        break;

                    case '"':
                        if(i > start)
                            tokens.push_back(Token(input + start, i - start));
                        state = IN_STR;
                        start = i + 1;
                        break;

                    case '|':
                    case '>':
                    case '<':
                    case '$':
                    case '=':
                        // in the variable part, "=" means assignment; otherwise it's just a
                        // character without meaning
                        if(in_vars || c != '=') {
                            // if a new command starts, there can be new variables
                            if(c == '|' || c == ';')
                                in_vars = true;
                            // remember if we've seen a "=" to detect the end of the variable part
                            else if(c == '=')
                                seen_eq = true;
                            if(i > start)
                                tokens.push_back(Token(input + start, i - start));
                            tokens.push_back(Token::from_char(c));
                            start = i + 1;
                        }
                        break;
                }
                break;

            case IN_STR:
                if(c == '"') {
                    tokens.push_back(Token(input + start, i - start));
                    state = DEFAULT;
                    start = i + 1;
                }
                break;
        }
    }

    // anything left?
    if(start < len)
        tokens.push_back(Token(input + start, len - start));

    return tokens;
}
