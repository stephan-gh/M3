/*
 * Copyright (C) 2021, Tendsin Mende <tendsin.mende@mailbox.tu-dresden.de>
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

use crate::importer::{DbOp, Package};
use hashbrown::HashMap;
use m3::vec::Vec;
use m3::{col::String, tcu::TCU};

pub struct KeyValueStore {
    pub tables: HashMap<u64, HashMap<String, String>>,
    t_insert: u64,
    t_delete: u64,
    t_read: u64,
    t_scan: u64,
    t_update: u64,
    n_insert: u64,
    n_delete: u64,
    n_read: u64,
    n_scan: u64,
    n_update: u64,
}

impl KeyValueStore {
    pub fn new() -> Self {
        KeyValueStore {
            tables: HashMap::new(),
            t_insert: 0,
            t_delete: 0,
            t_read: 0,
            t_scan: 0,
            t_update: 0,
            n_insert: 0,
            n_delete: 0,
            n_read: 0,
            n_scan: 0,
            n_update: 0,
        }
    }

    /// Executes the packages action, might return a response.
    pub fn execute(&mut self, pkg: Package) -> Result<Option<Vec<u8>>, ()> {
        match pkg.op {
            const { DbOp::Insert as u8 } => {
                let start = TCU::nanotime();
                self.insert(pkg);
                self.t_insert += TCU::nanotime() - start;
                self.n_insert += 1;
                Ok(None)
            },
            const { DbOp::Delete as u8 } => {
                let start = TCU::nanotime();
                self.delete(pkg);
                self.t_delete += TCU::nanotime() - start;
                self.n_delete += 1;
                Ok(None)
            },
            const { DbOp::Read as u8 } => {
                let start = TCU::nanotime();
                let res = if let Some(return_pairs) = self.read(pkg) {
                    // Convert into byte array of the form
                    let mut return_buffer = Vec::with_capacity(return_pairs.len() * 3);
                    for (k, v) in return_pairs {
                        return_buffer.push(k.len() as u8);
                        return_buffer.push(v.len() as u8);
                        return_buffer.append(&mut k.into_bytes());
                        return_buffer.append(&mut v.into_bytes())
                    }

                    Ok(Some(return_buffer))
                }
                else {
                    Ok(None)
                };

                self.t_read += TCU::nanotime() - start;
                self.n_read += 1;
                res
            },
            const { DbOp::Scan as u8 } => {
                let start = TCU::nanotime();

                let _res = self.scan(pkg);
                // NOTE we could package a return buffer simimar to the READ operation and return it.

                self.t_scan += TCU::nanotime() - start;
                self.n_scan += 1;
                Ok(None)
            },
            const { DbOp::Update as u8 } => {
                let start = TCU::nanotime();
                self.update(pkg);
                self.t_update += TCU::nanotime() - start;
                self.n_update += 1;
                Ok(None)
            },
            _ => {
                println!("Unknown Op(Code: {})!", pkg.op);
                Err(())
            },
        }
    }

    fn insert(&mut self, op: Package) {
        // make sure there is such a table
        if self.tables.get(&op.key).is_none() {
            self.tables.insert(op.key, HashMap::new());
        }

        let table = self.tables.get_mut(&op.key).unwrap();
        for (k, v) in op.kv_pairs.into_iter() {
            table.insert(k, v);
        }
    }

    fn delete(&mut self, op: Package) {
        if let Some(_table) = self.tables.get_mut(&op.key) {
            println!("Delete not yet implemented");
        }
        else {
            println!("Server: WARNING: Tried to delete from unknown table");
        }
    }

    fn read(&mut self, op: Package) -> Option<Vec<(String, String)>> {
        if let Some(table) = self.tables.get(&op.key) {
            // If the k,v pairs are empty, this means "all fields" should be read, otherwise read
            // only the specified ones.
            if op.kv_pairs.len() == 0 {
                Some(
                    table
                        .iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect::<Vec<_>>(),
                )
            }
            else {
                let mut reads = Vec::new();
                for (k, _v) in op.kv_pairs {
                    if let Some(result) = table.get(&k) {
                        reads.push((k, result.clone()));
                    }
                }
                Some(reads)
            }
        }
        else {
            println!("Server: WARNING: Read on unknown table");
            None
        }
    }

    fn scan(&mut self, op: Package) -> Vec<Vec<(String, String)>> {
        // In scan we walk over the records starting at op.key and going until op.key +
        // op.scan_length. For each table we add each value thats available for any key in kv_pairs,
        // or we add every value if kv_pairs is empty.
        let mut results = Vec::new();

        // Not realy correct since the Hashmap is not in order...
        // TODO order hashmap to start at correct offset?
        let mut num_scans = 0;
        for (tk, table) in self.tables.iter() {
            if num_scans > op.scan_length {
                break;
            }

            if *tk >= op.key {
                let mut sub_results = Vec::new();

                if op.kv_pairs.len() > 0 {
                    for (k, _v) in &op.kv_pairs {
                        if let Some(val) = table.get(k) {
                            sub_results.push((k.clone(), val.clone()));
                        }
                    }
                }
                else {
                    // Should read all keys
                    for (k, v) in table.iter() {
                        sub_results.push((k.clone(), v.clone()))
                    }
                }

                if sub_results.len() > 0 {
                    results.push(sub_results);
                }
                num_scans += 1;
            }
        }

        results
    }

    fn update(&mut self, op: Package) {
        if let Some(table) = self.tables.get_mut(&op.key) {
            for (k, v) in op.kv_pairs {
                if let Some(value) = table.get_mut(&k) {
                    *value = v;
                }
                else {
                    table.insert(k, v);
                }
            }
        }
        else {
            println!("Server: WARNING: Update on unknown table");
        }
    }

    pub fn print_stats(&self, num_ops: usize) {
        println!("    Key Value Database Timings for {} operations:", num_ops);
        println!(
            "        Insert: {}ns,\t avg_time: {}ns",
            self.t_insert,
            if self.n_insert > 0 { self.t_insert / self.n_insert } else { 0 }
        );
        println!(
            "        Delete: {}ns,\t avg_time: {}ns",
            self.t_delete,
            if self.n_delete > 0 { self.t_delete / self.n_delete } else { 0 }
        );
        println!(
            "        Read:   {}ns,\t avg_time: {}ns",
            self.t_read,
            if self.n_read > 0 { self.t_read / self.n_read } else { 0 }
        );
        println!(
            "        Update: {}ns,\t avg_time: {}ns",
            self.t_update,
            if self.n_update > 0 { self.t_update / self.n_update } else { 0 }
        );
        println!(
            "        Scan:   {}ns,\t avg_time: {}ns",
            self.t_scan,
            if self.n_scan > 0 { self.t_scan / self.n_scan } else { 0 }
        );
    }
}
