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

#pragma once

#include <base/Common.h>
#include <base/stream/Format.h>

#include <algorithm>
#include <initializer_list>
#include <memory>
#include <string>
#include <vector>

#include "Tokenizer.h"

class Parser {
public:
    struct PrevToken {
        PrevToken(Parser *parser) : parser(parser) {
        }

        void format(m3::OStream &os, const m3::FormatSpecs &) const {
            using namespace m3;
            if(parser->_token > 0)
                format_to(os, " after "_cf, parser->_tokens[parser->_token - 1]);
        }

        Parser *parser;
    };

public:
    template<typename T>
    class List {
    public:
        typedef std::vector<std::unique_ptr<T>> list_type;

        explicit List() : _list() {
        }

        size_t size() const {
            return _list.size();
        }

        const std::unique_ptr<T> &get(size_t idx) const {
            return _list[idx];
        }

        const typename list_type::const_iterator cbegin() const {
            return _list.cbegin();
        }
        const typename list_type::const_iterator cend() const {
            return _list.cend();
        }

        void add(std::unique_ptr<T> &&e) {
            _list.push_back(std::move(e));
        }
        void insert(size_t i, std::unique_ptr<T> &&e) {
            auto pos = std::next(_list.begin(), static_cast<ssize_t>(i));
            _list.insert(pos, std::move(e));
        }
        void replace(size_t i, std::unique_ptr<T> &&e) {
            _list[i] = std::move(e);
        }
        void remove(size_t i) {
            auto pos = std::next(_list.begin(), static_cast<ssize_t>(i));
            _list.erase(pos);
        }

    private:
        list_type _list;
    };

    class Expr {
    public:
        explicit Expr(const std::string &name, int is_var) : _name(name), _is_var(is_var) {
        }

        bool is_var() const {
            return _is_var;
        }
        const std::string &name() const {
            return _name;
        }

    private:
        std::string _name;
        bool _is_var;
    };

    class ArgList : public List<Expr> {
    public:
        explicit ArgList() : List() {
        }
    };

    class RedirList {
    public:
        explicit RedirList() : _fds() {
        }

        const std::unique_ptr<Expr> &std_in() const {
            return _fds[0];
        }
        const std::unique_ptr<Expr> &std_out() const {
            return _fds[1];
        }
        void std_in(std::unique_ptr<Expr> &&path) {
            _fds[0] = std::move(path);
        }
        void std_out(std::unique_ptr<Expr> &&path) {
            _fds[1] = std::move(path);
        }

    private:
        std::unique_ptr<Expr> _fds[2];
    };

    class Var {
    public:
        explicit Var(const std::string &name, std::unique_ptr<Expr> &&value)
            : _name(name),
              _value(std::move(value)) {
        }

        const std::string &name() const {
            return _name;
        }
        const std::unique_ptr<Expr> &value() const {
            return _value;
        }

    private:
        std::string _name;
        std::unique_ptr<Expr> _value;
    };

    class VarList : public List<Var> {
    public:
        explicit VarList() : List() {
        }
    };

    class Command {
    public:
        explicit Command(std::unique_ptr<VarList> &&vars, std::unique_ptr<ArgList> &&args,
                         std::unique_ptr<RedirList> &&redirs)
            : _vars(std::move(vars)),
              _args(std::move(args)),
              _redirs(std::move(redirs)) {
        }

        std::unique_ptr<ArgList> &args() {
            return _args;
        }
        const std::unique_ptr<VarList> &vars() const {
            return _vars;
        }
        const std::unique_ptr<ArgList> &args() const {
            return _args;
        }
        const std::unique_ptr<RedirList> &redirections() const {
            return _redirs;
        }

    private:
        std::unique_ptr<VarList> _vars;
        std::unique_ptr<ArgList> _args;
        std::unique_ptr<RedirList> _redirs;
    };

    class CmdList : public List<Command> {
    public:
        explicit CmdList() : List() {
        }
    };

    explicit Parser(std::vector<Token> &&tokens) : _tokens(std::move(tokens)), _token() {
    }

    std::unique_ptr<CmdList> parse();

private:
    const Token *token(size_t off) const;
    bool expr_follows() const;

    const Token &expect_token(std::initializer_list<TokenType> tokens);

    std::unique_ptr<Command> parse_command();
    std::unique_ptr<VarList> parse_vars();
    std::unique_ptr<ArgList> parse_args();
    std::unique_ptr<Expr> parse_expr();
    std::unique_ptr<RedirList> parse_redirections();

public:
    std::vector<Token> _tokens;
    size_t _token;
};

namespace m3 {

template<>
struct Formatter<std::initializer_list<TokenType>> {
    constexpr void format(OStream &os, const FormatSpecs &,
                          const std::initializer_list<TokenType> &t) const {
        for(auto it = t.begin(); it != t.end(); ++it) {
            format_to(os, "{}"_cf, (int)*it);
            if(it + 1 != t.end())
                format_to(os, ", or "_cf);
        }
    }
};

}
