/*
 * Copyright (C) 2020-2021 Nils Asmussen, Barkhausen Institut
 * Copyright (C) 2019-2020, Tendsin Mende <tendsin@protonmail.com>
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

use crate::data::{DirEntry, INodeRef, DIR_ENTRY_LEN};
use crate::ops::inodes;

use base::io::LogFlags;
use m3::errors::{Code, Error};

/// Creates a link in directory `dir` with given name pointing to `inode`.
///
/// Assumes that no entry with given name already exists!
pub fn create(dir: &INodeRef, name: &str, inode: &INodeRef) -> Result<(), Error> {
    log!(
        LogFlags::FSLinks,
        "links::create(dir={}, name={}, inode={})",
        dir.inode,
        name,
        inode.inode,
    );

    let mut created = false;
    let new_entry_size = DIR_ENTRY_LEN + name.len();

    'search_loop: for ext in dir.extent_iter() {
        for mut block in ext.block_iter() {
            let mut off = 0;
            let end = crate::superblock().block_size as usize;
            while off < end {
                let entry = DirEntry::from_buffer_mut(&mut block, off);

                let rem = entry.next - entry.size() as u32;
                if rem >= new_entry_size as u32 {
                    // change current entry
                    entry.next = entry.size() as u32;
                    let entry_next = entry.next;

                    // create new entry behind it
                    let new_entry =
                        DirEntry::from_buffer_mut(&mut block, off + entry_next as usize);

                    new_entry.set_name(name);
                    new_entry.nodeno = inode.inode;
                    new_entry.next = rem;

                    created = true;
                    break 'search_loop;
                }

                off += entry.next as usize;
            }
        }
    }

    // no suitable space found; extend directory
    if !created {
        let mut indir = None;
        let ext = inodes::get_extent(dir, dir.extents as usize, &mut indir, true)?;

        // insert one block extent
        let ext_range = inodes::create_extent(Some(dir), 1)?;
        *ext.as_mut() = ext_range;

        // put entry at the beginning of the block
        let start = ext.start;
        let mut block = crate::meta_buffer_mut().get_block(start)?;
        let new_entry = DirEntry::from_buffer_mut(&mut block, 0);
        new_entry.set_name(name);
        new_entry.nodeno = inode.inode;
        new_entry.next = crate::superblock().block_size;
    }

    inode.as_mut().links += 1;
    Ok(())
}

/// Removes the link with given name from `dir`
///
/// If `deny_dir` is true, the function fails if the link points to a directory.
pub fn remove(dir: &INodeRef, name: &str, deny_dir: bool) -> Result<(), Error> {
    log!(
        LogFlags::FSLinks,
        "links::remove(dir={}, name={}, deny_dir={})",
        dir.inode,
        name,
        deny_dir
    );

    for ext in dir.extent_iter() {
        for mut block in ext.block_iter() {
            let mut prev_off = 0;
            let mut off = 0;
            let end = crate::superblock().block_size as usize;
            while off < end {
                // TODO marking all blocks dirty here is suboptimal
                let entry = DirEntry::from_buffer_mut(&mut block, off);

                if entry.name() == name {
                    // if we're not removing a dir, we're coming from unlink(). in this case,
                    // directories are not allowed
                    let inode = inodes::get(entry.nodeno)?;
                    if deny_dir && inode.mode.is_dir() {
                        return Err(Error::new(Code::IsDir));
                    }

                    let entry_next = entry.next;

                    // remove entry by skipping over it
                    if off > 0 {
                        let prev = DirEntry::from_buffer_mut(&mut block, prev_off);
                        prev.next += entry_next;
                    }
                    // copy the next entry back, if there is any
                    else {
                        let next_off = off + entry_next as usize;
                        if next_off < end {
                            let (cur_entry, next_entry) = DirEntry::two_from_buffer_mut(
                                &mut block,
                                off,
                                off + entry_next as usize,
                            );

                            let dist = cur_entry.next;
                            cur_entry.next = next_entry.next;
                            cur_entry.nodeno = next_entry.nodeno;

                            cur_entry.set_name(next_entry.name());
                            cur_entry.next = dist + next_entry.next;
                        }
                    }

                    // reduce links and free if necessary
                    inodes::decrease_links(&inode)?;

                    return Ok(());
                }

                prev_off = off;
                off += entry.next as usize;
            }
        }
    }

    Err(Error::new(Code::NoSuchFile))
}
