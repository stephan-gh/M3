use crate::buffer::Buffer;
use crate::data::*;
use crate::internal::*;
use crate::sess::*;
use m3::errors::{Code, Error};

pub struct Links {}

impl Links {
    pub fn create(
        req: &mut Request,
        dir: LoadedInode,
        name: &str,
        inode: LoadedInode,
    ) -> Result<(), Error> {
        log!(
            crate::LOG_DEF,
            "Links::create(dir={}, name={}, inode={})",
            { dir.inode().inode },
            name,
            { inode.inode().inode }
        ); // {} needed because of packed inode struct
        let org_used = req.used_meta();
        let mut indir = vec![];

        let mut rem = 0;
        let mut new_entry = None;

        'search_loop: for ext_idx in 0..dir.inode().extents {
            let ext = INodes::get_extent(req, dir.clone(), ext_idx as usize, &mut indir, false)?;

            for bno in ext.into_iter() {
                // This is the block for all entries that are within this block
                let dir_entry_data_ref = crate::hdl().metabuffer().get_block(req, bno, false)?;
                let mut entry_location = 0;
                // Max offset into the buffer at which a entry could be. Is anyways incorrect since each name has a dynamic length.
                let entry_location_end = crate::hdl().superblock().block_size;
                // Iter over the entries, at the end always increment the ptr offset by entry.next
                while entry_location < entry_location_end {
                    let entry = DirEntry::from_buffer_mut(
                        dir_entry_data_ref.clone(),
                        entry_location as usize,
                    );

                    rem = entry.next - entry.size() as u32;
                    // This happens if we can embed the new dir-entry between this one and the "next"
                    if rem >= entry.size() as u32 {
                        // change previous entry
                        entry.next = entry.size() as u32;

                        // create new entry
                        new_entry = Some(DirEntry::from_buffer_mut(
                            dir_entry_data_ref,
                            (entry_location + entry.next) as usize,
                        ));

                        crate::hdl().metabuffer().mark_dirty(bno);
                        req.pop_metas(req.used_meta() - org_used);

                        break 'search_loop;
                    }

                    // Go to next entry
                    entry_location = entry_location + entry.next;
                }
                req.pop_meta();
            }
            req.pop_metas(req.used_meta() - org_used);
        }

        // Check if a suitable space was found, otherwise extend directory
        let entry = if let Some(e) = new_entry {
            e
        }
        else {
            // Create new
            let ext = INodes::get_extent(
                req,
                dir.clone(),
                dir.inode().extents as usize,
                &mut indir,
                true,
            )?;

            // Insert one block extent
            INodes::fill_extent(req, Some(dir), &ext, 1, 1)?;

            // put entry at the beginning of the block
            rem = crate::hdl().superblock().block_size;
            let start = *ext.start();
            DirEntry::from_buffer_mut(
                crate::hdl().metabuffer().get_block(req, start, true)?,
                0,
            )
        };

        // write entry
        entry.set_name(name);
        entry.nodeno = inode.inode().inode;
        entry.next = rem;

        inode.inode().links += 1;
        INodes::mark_dirty(req, inode.inode().inode);
        Ok(())
    }

    pub fn remove(
        req: &mut Request,
        dir: LoadedInode,
        name: &str,
        is_dir: bool,
    ) -> Result<(), Error> {
        let org_used = req.used_meta();
        let mut indir = vec![];

        log!(
            crate::LOG_DEF,
            "links::remove(name={}, is_dir={})",
            name,
            is_dir
        );
        for ext_idx in 0..dir.inode().extents {
            let ext = INodes::get_extent(req, dir.clone(), ext_idx as usize, &mut indir, false)?;
            for bno in ext.into_iter() {
                // This is the block for all entries that are within this block
                let dir_entry_data_ref = crate::hdl().metabuffer().get_block(req, bno, false)?;
                let mut entry_location = 0;
                // Max offset into the buffer at which a entry could be. Is anyways incorrect since each name has a dynamic length.
                let entry_location_end = crate::hdl().superblock().block_size;

                // previouse entry
                let mut prev: Option<&'static mut DirEntry> = None;

                // Iter over the entries, at the end always increment the ptr offset by entry.next
                while entry_location < entry_location_end {
                    let entry = DirEntry::from_buffer_mut(
                        dir_entry_data_ref.clone(),
                        entry_location as usize,
                    );

                    if entry.name() == name {
                        // if we are not removing a dir, we are coming from unlink(). in this case, directories
                        // are not allowed
                        let inode = INodes::get(req, entry.nodeno)?;
                        if !is_dir && crate::internal::is_dir(inode.inode().mode) {
                            req.pop_metas(req.used_meta() - org_used);
                            return Err(Error::new(Code::IsDir));
                        }

                        // remove entry by skipping over it
                        if let Some(p) = prev {
                            p.next += entry.next;
                        }
                        else {
                            // copy the next entry back, if there is any
                            let next_location = entry_location as usize + entry.next as usize;
                            let next_entry = DirEntry::from_buffer_mut(
                                dir_entry_data_ref,
                                next_location,
                            );

                            if next_location < entry_location_end as usize {
                                let dist = entry.next;
                                // Copy data over
                                entry.next = next_entry.next;
                                entry.nodeno = next_entry.nodeno;

                                entry.set_name(next_entry.name());
                                entry.next = dist + next_entry.next;
                            }
                        }
                        crate::hdl().metabuffer().mark_dirty(bno);
                        // reduce links and free if necessary
                        if (inode.inode().links - 1) == 0 {
                            let ino = inode.inode().inode;
                            crate::hdl().files().delete_file(ino)?;
                        }

                        req.pop_metas(req.used_meta() - org_used);
                        return Ok(());
                    }
                    // Go to next entry
                    entry_location = entry_location + entry.next;
                    // Update pref
                    prev = Some(entry);
                }

                req.pop_meta();
            }
            req.pop_metas(req.used_meta() - org_used);
        }

        Err(Error::new(Code::NoSuchFile))
    }
}
