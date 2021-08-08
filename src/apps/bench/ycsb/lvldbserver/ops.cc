/*
 * Copyright (C) 2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <base/TCU.h>

#include <m3/stream/Standard.h>
#include <m3/Exception.h>

#include <sstream>

#include "ops.h"

#define DEBUG 0

Executor *Executor::create(const char *db) {
    return new LevelDBExecutor(db);
}

LevelDBExecutor::LevelDBExecutor(const char *db)
    : _t_insert(),
      _t_read(),
      _t_scan(),
      _t_update(),
      _n_insert(),
      _n_read(),
      _n_scan(),
      _n_update() {
    leveldb::Options options;
    options.create_if_missing = true;
    leveldb::Status status = leveldb::DB::Open(options, db, &_db);
    if(!status.ok())
        VTHROW(m3::Errors::INV_ARGS, "Unable to open/create DB '" << db
            << "': " << status.ToString().c_str());
}

LevelDBExecutor::~LevelDBExecutor() {
    delete _db;
}

void LevelDBExecutor::reset_stats() {
    _n_insert = 0;
    _n_read = 0;
    _n_scan = 0;
    _n_update = 0;
    _t_insert = 0;
    _t_read = 0;
    _t_scan = 0;
    _t_update = 0;
}

void LevelDBExecutor::print_stats(size_t num_ops) {
    uint64_t avg;
    m3::cout << "    Key Value Database Timings for " << num_ops << " operations:\n";

    avg = _n_insert > 0 ? _t_insert / _n_insert : 0;
    m3::cout << "        Insert: " << _t_insert << " cycles,\t avg_time: " << avg << " cycles\n",

    avg = _n_read > 0 ? _t_read / _n_read : 0;
    m3::cout << "        Read:   " << _t_read << " cycles,\t avg_time: " << avg << " cycles\n";

    avg = _n_update > 0 ? _t_update / _n_update : 0;
    m3::cout << "        Update: " << _t_update << " cycles,\t avg_time: " << avg << " cycles\n";

    avg = _n_scan > 0 ? _t_scan / _n_scan : 0;
    m3::cout << "        Scan:   " << _t_scan << " cycles,\t avg_time: " << avg << " cycles\n";
}

void LevelDBExecutor::execute(Package &pkg) {
#if DEBUG > 0
    m3::cout << "Executing operation " << (int)pkg.op << " with table " << (int)pkg.table;
    m3::cout << "  num_kvs=" << (int)pkg.num_kvs << ", key=" << pkg.key;
    m3::cout << ", scan_length=" << pkg.scan_length << "\n";
#endif
#if DEBUG > 1
    for(auto &pair : pkg.kv_pairs)
        m3::cout << "  key='field" << pair.first.c_str() << "' val='" << pair.second.c_str() << "'\n";
#endif

    switch(pkg.op) {
        case Operation::INSERT: {
            uint64_t start = m3::CPU::elapsed_cycles();
            exec_insert(pkg);
            _t_insert += m3::CPU::elapsed_cycles() - start;
            _n_insert++;
            break;
        }

        case Operation::UPDATE: {
            uint64_t start = m3::CPU::elapsed_cycles();
            exec_insert(pkg);
            _t_update += m3::CPU::elapsed_cycles() - start;
            _n_update++;
            break;
        }

        case Operation::READ: {
            uint64_t start = m3::CPU::elapsed_cycles();
            auto vals = exec_read(pkg);
            for(auto &pair : vals) {
                (void)pair;
#if DEBUG > 1
                m3::cout << "  found '" << pair.first.c_str()
                         << "' -> '" << pair.second.c_str() << "'\n";
#endif
            }
            _t_read += m3::CPU::elapsed_cycles() - start;
            _n_read++;
            break;
        }

        case Operation::SCAN: {
            uint64_t start = m3::CPU::elapsed_cycles();
            auto vals = exec_scan(pkg);
            for(auto &pair : vals) {
                (void)pair;
#if DEBUG > 1
                m3::cout << "  found '" << pair.first.c_str()
                         << "' -> '" << pair.second.c_str() << "'\n";
#endif
            }
            _t_scan += m3::CPU::elapsed_cycles() - start;
            _n_scan++;
            break;
        }

        case Operation::DELETE:
            m3::cerr << "DELETE is not supported\n";
            break;

        case 0: break;
    }
}

static std::string pack_key(uint64_t key, const std::string &field, const char *prefix) {
    std::ostringstream key_field;
    key_field << key << "/" << prefix << field;
    return key_field.str();
}

static std::pair<uint64_t, std::string> unpack_key(const std::string &key_field) {
    size_t pos = 0;
    uint64_t key = static_cast<uint64_t>(std::stoll(key_field, &pos));
    std::string field = key_field.substr(pos + 1);
    return std::make_pair(key, field);
}

void LevelDBExecutor::exec_insert(Package &pkg) {
    leveldb::WriteOptions writeOptions;
    for(auto &pair : pkg.kv_pairs) {
        auto key = pack_key(pkg.key, pair.first, "field");
#if DEBUG > 1
        m3::cerr << "Setting '" << key.c_str() << "' to '" << pair.second.c_str() << "'\n";
#endif
        _db->Put(writeOptions, key, pair.second);
    }
}

std::vector<std::pair<std::string, std::string>> LevelDBExecutor::exec_read(Package &pkg) {
    std::vector<std::pair<std::string, std::string>> res;
    // If the k,v pairs are empty, this means "all fields" should be read
    if(pkg.kv_pairs.empty()) {
        leveldb::Iterator *it = _db->NewIterator(leveldb::ReadOptions());
        for (it->SeekToFirst(); it->Valid(); it->Next()) {
            std::istringstream is(it->key().ToString());
            uint64_t key;
            is >> key;
            if(key == pkg.key) {
                std::string field;
                is >> field;
                res.push_back(std::make_pair(field, it->value().ToString()));
            }
        }
    }
    else {
        for(auto &pair : pkg.kv_pairs) {
            auto key = pack_key(pkg.key, pair.first, "");
            std::string value;
            auto s = _db->Get(leveldb::ReadOptions(), key, &value);
            if (s.ok())
                res.push_back(std::make_pair(pair.first, value));
            else
                m3::cerr << "Unable to find key '" << key.c_str() << "'\n";
        }
    }
    return res;
}

static bool take_field(Package &pkg, const std::string &field) {
    if(pkg.kv_pairs.empty())
        return true;
    for(auto &pair : pkg.kv_pairs) {
        if(pair.first == field)
            return true;
    }
    return false;
}

std::vector<std::pair<std::string, std::string>> LevelDBExecutor::exec_scan(Package &pkg) {
    std::vector<std::pair<std::string, std::string>> res;
    size_t rem = pkg.scan_length;
    uint64_t last_key = 0;
    leveldb::Iterator *it = _db->NewIterator(leveldb::ReadOptions());
    if(pkg.kv_pairs.size() == 1) {
        auto key = pack_key(pkg.key, pkg.kv_pairs.front().first, "");
        it->Seek(key);
    }
    else
        it->SeekToFirst();
    for (; rem > 0 && it->Valid(); it->Next()) {
        auto pair = unpack_key(it->key().ToString());
        if(pair.first >= pkg.key) {
            if(take_field(pkg, pair.second)) {
                res.push_back(std::make_pair(pair.second, it->value().ToString()));
                if(last_key && last_key != pair.first)
                    rem--;
            }
            last_key = pair.first;
        }
    }
    return res;
}
