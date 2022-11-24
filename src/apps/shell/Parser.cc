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

#include "Parser.h"

#include <m3/stream/Standard.h>

#include <algorithm>
#include <exception>
#include <memory>

using namespace m3;

const Token &Parser::expect_token(std::initializer_list<TokenType> tokens) {
    const Token *cur = token(0);
    if(!cur)
        vthrow(Errors::SUCCESS, "Missing token{}; expected {}"_cf, PrevToken(this), tokens);

    for(auto it = tokens.begin(); it != tokens.end(); ++it) {
        if(cur->type() == *it) {
            _token++;
            return *cur;
        }
    }

    vthrow(Errors::SUCCESS, "Unexpected token{}; expected {}"_cf, PrevToken(this), tokens);
}

const Token *Parser::token(size_t off) const {
    if(_token + off >= _tokens.size())
        return nullptr;
    return &_tokens[_token + off];
}

bool Parser::expr_follows() const {
    const Token *cur = token(0);
    return cur && (cur->type() == TokenType::STRING || cur->type() == TokenType::DOLLAR);
}

std::unique_ptr<Parser::Expr> Parser::parse_expr() {
    const Token &cur = expect_token({TokenType::DOLLAR, TokenType::STRING});
    if(cur.type() == TokenType::STRING)
        return std::make_unique<Expr>(cur.string(), false);
    else {
        const Token &var_name = expect_token({TokenType::STRING});
        return std::make_unique<Expr>(var_name.string(), true);
    }
}

std::unique_ptr<Parser::VarList> Parser::parse_vars() {
    auto list = std::make_unique<VarList>();
    while(1) {
        const Token *cur = token(0);
        const Token *next = token(1);
        if(cur && next && cur->type() == TokenType::STRING && next->type() == TokenType::ASSIGN) {
            _token += 2;
            list->add(std::make_unique<Var>(cur->string(), parse_expr()));
        }
        else
            break;
    }
    return list;
}

std::unique_ptr<Parser::ArgList> Parser::parse_args() {
    auto list = std::make_unique<ArgList>();
    list->add(parse_expr());
    while(expr_follows())
        list->add(parse_expr());
    return list;
}

std::unique_ptr<Parser::RedirList> Parser::parse_redirections() {
    auto list = std::make_unique<RedirList>();
    const Token *cur;
    while(((cur = token(0))) != nullptr) {
        switch(cur->type()) {
            case TokenType::LESS_THAN:
                _token++;
                list->std_in(parse_expr());
                break;
            case TokenType::GREATER_THAN:
                _token++;
                list->std_out(parse_expr());
                break;
            default: goto done;
        }
    }
done:
    return list;
}

std::unique_ptr<Parser::Command> Parser::parse_command() {
    auto vars = parse_vars();
    auto args = parse_args();
    auto redirs = parse_redirections();
    return std::make_unique<Command>(std::move(vars), std::move(args), std::move(redirs));
}

std::unique_ptr<Parser::CmdList> Parser::parse() {
    auto list = std::make_unique<CmdList>();
    while(1) {
        std::unique_ptr<Command> cmd = parse_command();
        list->add(std::move(cmd));

        if(!token(0))
            break;

        expect_token({TokenType::PIPE});
    }
    return list;
}
