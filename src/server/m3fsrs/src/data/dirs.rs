use crate::data::{INodes, Links};
use crate::internal::{DirEntryIterator, FileMode, INodeRef, InodeNo};

use m3::errors::{Code, Error};

pub struct Dirs;

impl Dirs {
    fn find_entry(inode: &INodeRef, name: &str) -> Result<InodeNo, Error> {
        let mut indir = None;

        for ext_idx in 0..inode.extents {
            let ext = INodes::get_extent(inode, ext_idx as usize, &mut indir, false)?;
            for bno in ext.blocks() {
                let mut block = crate::hdl().metabuffer().get_block(bno, false)?;
                let entry_iter = DirEntryIterator::from_block(&mut block);
                while let Some(entry) = entry_iter.next() {
                    if entry.name() == name {
                        return Ok(entry.nodeno);
                    }
                }
            }
        }

        Err(Error::new(Code::NoSuchFile))
    }

    pub fn search(path: &str, create: bool) -> Result<InodeNo, Error> {
        let ino = Self::do_search(path, create);
        log!(
            crate::LOG_DIRS,
            "dirs::search(path={}, create={}) -> {:?}",
            path,
            create,
            ino.as_ref().map_err(|e| e.code()),
        );
        ino
    }

    fn do_search(mut path: &str, create: bool) -> Result<InodeNo, Error> {
        // remove all leading /
        while path.starts_with('/') {
            path = &path[1..];
        }

        // root inode?
        if path == "" {
            return Ok(0);
        }

        // start at root inode with search
        let mut ino = 0;

        let (filename, inode) = loop {
            // get directory inode
            let inode = INodes::get(ino)?;

            // find directory entry
            let next_end = path.find('/').unwrap_or(path.len());
            let filename = &path[..next_end];
            let next_ino = Dirs::find_entry(&inode, filename);

            // walk to next path component start
            let mut end = &path[next_end..];
            while end.starts_with('/') {
                end = &end[1..];
            }

            if let Ok(nodeno) = next_ino {
                // if path is now empty, finish searching
                if end == "" {
                    return Ok(nodeno);
                }
                // continue with this directory
                ino = nodeno;
            }
            else {
                // cannot create new file if it's not the last path component
                if end != "" {
                    return Err(Error::new(Code::NoSuchFile));
                }

                // not found, maybe we want to create it
                break (filename, inode);
            }

            // to next path component
            path = end;
        };

        if create {
            // create inode and put link into directory
            let new_inode = INodes::create(FileMode::FILE_DEF)?;
            if let Err(e) = Links::create(&inode, filename, &new_inode) {
                crate::hdl().files().delete_file(new_inode.inode).ok();
                return Err(e);
            };
            return Ok(new_inode.inode);
        }

        Err(Error::new(Code::NoSuchFile))
    }

    pub fn create(path: &str, mode: FileMode) -> Result<(), Error> {
        let res = Self::do_create(path, mode);
        log!(
            crate::LOG_DIRS,
            "dirs::create(path={}, mode={:o}) -> {:?}",
            path,
            mode,
            res.as_ref().map_err(|e| e.code()),
        );
        res
    }

    fn do_create(path: &str, mode: FileMode) -> Result<(), Error> {
        // Split the path into the dir part and the base(name) part.
        // might have to change the dir into "." if the file is located at the root

        let (mut base, dir) = {
            let (base_slice, dir_slice) = crate::util::get_base_dir(path);
            (&path[base_slice], &path[dir_slice])
        };

        // If there is no base, we are at the root of the file system.
        if base == "" {
            base = "/";
        }

        let parent_ino = Dirs::search(base, false)?;

        // Ensure that the entry doesn't exist
        if Dirs::search(path, false).is_ok() {
            return Err(Error::new(Code::Exists));
        }

        let parinode = INodes::get(parent_ino)?;
        if let Ok(dirino) = INodes::create(FileMode::DIR_DEF | mode) {
            // create directory itself
            if let Err(e) = Links::create(&parinode, dir, &dirino) {
                crate::hdl().files().delete_file(dirino.inode).ok();
                return Err(e);
            }

            // create "." link
            if let Err(e) = Links::create(&dirino, ".", &dirino) {
                Links::remove(&parinode, dir, true).unwrap();
                crate::hdl().files().delete_file(dirino.inode).ok();
                return Err(e);
            }

            // create ".." link
            if let Err(e) = Links::create(&dirino, "..", &parinode) {
                Links::remove(&dirino, ".", true).unwrap();
                Links::remove(&parinode, dir, true).unwrap();
                crate::hdl().files().delete_file(dirino.inode).ok();
                return Err(e);
            }

            Ok(())
        }
        else {
            Err(Error::new(Code::NoSpace))
        }
    }

    pub fn remove(path: &str) -> Result<(), Error> {
        log!(crate::LOG_DIRS, "dirs::remove(path={})", path);

        let ino = Dirs::search(path, false)?;

        // it has to be a directory
        let inode = INodes::get(ino)?;
        if !inode.mode.is_dir() {
            return Err(Error::new(Code::IsNoDir));
        }

        // check whether it's empty
        let mut indir = None;

        for ext_idx in 0..inode.extents {
            let ext = INodes::get_extent(&inode, ext_idx as usize, &mut indir, false)?;
            for bno in ext.blocks() {
                let mut block = crate::hdl().metabuffer().get_block(bno, false)?;
                let entry_iter = DirEntryIterator::from_block(&mut block);
                while let Some(entry) = entry_iter.next() {
                    if entry.name() != "." && entry.name() != ".." {
                        return Err(Error::new(Code::DirNotEmpty));
                    }
                }
            }
        }

        // hardlinks to directories are not possible, thus we always have 2 ( . and ..)
        assert!(inode.links == 2, "expected 2 links, found {}", inode.links);
        inode.as_mut().links -= 1;

        // TODO if that fails, we have already reduced the link count!?
        Dirs::unlink(path, true)
    }

    pub fn link(old_path: &str, new_path: &str) -> Result<(), Error> {
        log!(
            crate::LOG_DIRS,
            "dirs::link(old_path={}, new_path={})",
            old_path,
            new_path
        );

        let oldino = Dirs::search(old_path, false)?;

        // it can't be a directory
        let old_inode = INodes::get(oldino)?;
        if old_inode.mode.is_dir() {
            return Err(Error::new(Code::IsDir));
        }

        // Split path into dir and base
        let (base, dir) = {
            let (base_slice, dir_slice) = crate::util::get_base_dir(new_path);
            (&new_path[base_slice], &new_path[dir_slice])
        };

        let baseino = Dirs::search(base, false)?;
        let base_ino = INodes::get(baseino)?;
        Links::create(&base_ino, dir, &old_inode)
    }

    pub fn unlink(path: &str, is_dir: bool) -> Result<(), Error> {
        log!(
            crate::LOG_DIRS,
            "dirs::unlink(path={}, is_dir={})",
            path,
            is_dir
        );

        let (base, dir) = {
            let (base_slice, dir_slice) = crate::util::get_base_dir(path);
            (&path[base_slice], &path[dir_slice])
        };

        let parino = Dirs::search(base, false)?;
        let parinode = INodes::get(parino)?;

        let res = Links::remove(&parinode, dir, is_dir);
        if is_dir && res.is_ok() {
            // decrement link count for parent inode by one
            parinode.as_mut().links -= 1;
        }

        res
    }
}
