use crate::data::*;
use crate::internal::*;
use crate::sess::request::Request;
use crate::util::*;

use m3::errors::*;

pub struct Dirs;

impl Dirs {
    fn find_entry(
        req: &mut Request,
        inode: LoadedInode,
        name: &str,
    ) -> Result<InodeNo, Error> {
        let org_used = req.used_meta();
        let mut indir = vec![];

        log!(
            crate::LOG_DEF,
            "dirs::find_entry(entry={} inode={}",
            name,
            { inode.inode().inode }
        ); //{} needed because of packed inode struct

        for ext_idx in 0..inode.inode().extents {
            let ext = INodes::get_extent(req, inode.clone(), ext_idx as usize, &mut indir, false)?;
            for bno in ext.into_iter() {
                for entry in
                    EntryIterator::from_block(crate::hdl().metabuffer().get_block(req, bno, false)?)
                {
                    if *entry.entry.borrow().name_length as usize == name.len()
                        && (name == entry.entry.borrow().name)
                    {
                        log!(
                            crate::LOG_DEF,
                            "Found entry with name: {}",
                            entry.entry.borrow().name
                        );
                        return Ok(*entry.entry.borrow().nodeno);
                    }
                }
                req.pop_meta();
            }
            req.pop_metas(req.used_meta() - org_used);
        }
        Err(Error::new(Code::NoSuchFile))
    }

    pub fn search(req: &mut Request, mut path: &str, create: bool) -> Result<InodeNo, Error> {
        log!(
            crate::LOG_DEF,
            "dirs::search(path={}, create={})",
            path,
            create
        );
        //Remove all leading /
        while path.starts_with('/') {
            path = &path[1..path.len()];
        }

        //Check if this is now the root node, if thats the case we can always return the root inode (0)
        if path == "" {
            return Ok(0);
        }

        //Start at root inode with search
        let mut ino = 0;
        let org_used = req.used_meta();

        let mut inode: Option<LoadedInode> = None;

        let mut counter_end = 0;
        let mut last_start = 0;
        let mut last_end = 0;
        while let Some((start, end)) = crate::util::next_start_end(path, counter_end) {
            inode = Some(INodes::get(req, ino)?);

            if ino != inode.as_ref().unwrap().inode().inode {
                log!(
                    crate::LOG_DEF,
                    "Inode numbers of wanted and loaded inode do not match!"
                );
            }

            if let Ok(nodeno) =
                Dirs::find_entry(req, inode.clone().unwrap(), &path[start..end])
            {
                //If path is now empty, finish searching,
                //Test for 1, since there might be  a rest /
                if (path.len() - end) <= 1 {
                    req.pop_metas(req.used_meta() - org_used);
                    return Ok(nodeno);
                }
                //Save the inode anyways if we want to create a inode here.
                ino = nodeno;
                req.pop_metas(req.used_meta() - org_used);
            }
            else {
                //No such entry, therefore break
                req.pop_meta();
                break;
            }

            counter_end = end + 1;
            //Carry them so we can create a new dir if create==true;
            last_start = start;
            last_end = end;
        }

        //Did not find correct one, check if we can create one
        if create {
            let (inode_name_start, inode_name_end) =
                if let Some((start, end)) = crate::util::next_start_end(path, counter_end) {
                    (start, end)
                }
                else {
                    log!(
                        crate::LOG_DEF,
                        concat!(
                            "While creating new inode, the rest path component was not long enough",
                            " for another component:\n",
                            " wanted to create for {}\n",
                            " whole rest was {}",
                        ),
                        &path[last_start..last_end],
                        &path[last_start..path.len()]
                    );
                    return Err(Error::new(Code::NoSuchFile));
                };

            //Create inode and put link into directory
            let new_inode = INodes::create(req, M3FS_IFREG | 0o0644)?;
            new_inode.inode().mode = 0o644; //be sure to have correct rights
            let namelen = path[inode_name_start..inode_name_end].len();
            if let Err(e) = Links::create(
                req,
                inode.unwrap().clone(),
                &path[inode_name_start..inode_name_end],
                namelen,
                new_inode.clone(),
            ) {
                crate::hdl().files().delete_file(new_inode.inode().inode).ok();
                return Err(e);
            };
            return Ok(new_inode.inode().inode);
        }

        Err(Error::new(Code::NoSuchFile))
    }

    pub fn create(req: &mut Request, path: &str, mode: Mode) -> Result<(), Error> {
        //Split the path into the dir part and the base(name) part.
        //might have to change the dir into "." if the file is located at the root

        log!(
            crate::LOG_DEF,
            "dirs::create(path={}, mode={:o})",
            path,
            mode
        );

        let (mut base, dir) = {
            let (base_slice, dir_slice) = crate::util::get_base_dir(path);
            (&path[base_slice], &path[dir_slice])
        };

        //If there is no base, we are at the root of the file system.
        if base == "" {
            base = "/";
        }

        let parent_ino = Dirs::search(req, base, false)?;

        //Ensure that the entry doesn't exist
        if Dirs::search(req, path, false).is_ok() {
            log!(
                crate::LOG_DEF,
                "Directory({}) exists, can't be created",
                path
            );
            return Err(Error::new(Code::Exists));
        }

        let parinode = INodes::get(req, parent_ino)?;
        if let Ok(dirino) = INodes::create(req, M3FS_IFDIR | (mode & 0x777)) {
            //Create directory itself
            if let Err(e) = Links::create(req, parinode.clone(), dir, dir.len(), dirino.clone()) {
                crate::hdl().files().delete_file(dirino.inode().inode).ok();
                return Err(e);
            }
            //Successfully created directory
            //create "." and ".."
            if let Err(e) = Links::create(req, dirino.clone(), ".", 1, dirino.clone()) {
                Links::remove(req, parinode.clone(), dir, dir.len(), true).unwrap();
                crate::hdl().files().delete_file(dirino.inode().inode).ok();
                return Err(e);
            }
            //created ., now ..
            if let Err(e) = Links::create(req, dirino.clone(), "..", 2, parinode.clone()) {
                Links::remove(req, dirino.clone(), ".", 1, true).unwrap();
                Links::remove(req, parinode.clone(), dir, dir.len(), true).unwrap();
                crate::hdl().files().delete_file(dirino.inode().inode).ok();
                return Err(e);
            }
            //Everything created successful, therefore return
            return Ok(());
        }
        else {
            return Err(Error::new(Code::NoSpace));
        }
    }

    pub fn remove(req: &mut Request, path: &str) -> Result<(), Error> {
        log!(crate::LOG_DEF, "dirs::remove(path={})", path);

        let ino = Dirs::search(req, path, false)?;

        //it has to be a directory
        let inode = INodes::get(req, ino)?;
        if !is_dir(inode.inode().mode) {
            return Err(Error::new(Code::IsNoDir));
        }

        //check whether it's empty
        let org_used = req.used_meta();
        let mut indir = vec![];

        for ext_idx in 0..inode.inode().extents {
            let ext = INodes::get_extent(req, inode.clone(), ext_idx as usize, &mut indir, false)?;
            for bno in ext.into_iter() {
                for entry in
                    EntryIterator::from_block(crate::hdl().metabuffer().get_block(req, bno, false)?)
                {
                    if !(*entry.entry.borrow().name_length == 1 && entry.entry.borrow().name == ".")
                        && !(*entry.entry.borrow().name_length == 2
                            && entry.entry.borrow().name == "..")
                    {
                        req.pop_metas(req.used_meta() - org_used);
                        return Err(Error::new(Code::DirNotEmpty));
                    }
                }
                req.pop_meta();
            }
            req.pop_metas(req.used_meta() - org_used);
        }

        // hardlinks to directories are not possible, thus we always have 2 ( . and ..)
        assert!(
            inode.inode().links == 2,
            "Dir links should be 2 before removing but where {}!",
            { inode.inode().links }
        );
        // ensure that the inode is removed
        inode.inode().links -= 1;
        // TODO if that fails, we have already reduced the link count!?
        Dirs::unlink(req, path, true)
    }

    pub fn link(req: &mut Request, old_path: &str, new_path: &str) -> Result<(), Error> {
        log!(
            crate::LOG_DEF,
            "dirs::link(old_path={}, new_path={})",
            old_path,
            new_path
        );

        let oldino = Dirs::search(req, old_path, false)?;

        //it can't be a directory
        let old_inode = INodes::get(req, oldino)?;
        if is_dir(old_inode.inode().mode) {
            return Err(Error::new(Code::IsDir));
        }

        //Split path into dir and base
        let (base, dir) = {
            let (base_slice, dir_slice) = crate::util::get_base_dir(new_path);
            (&new_path[base_slice], &new_path[dir_slice])
        };

        let baseino = Dirs::search(req, base, false)?;
        let base_ino = INodes::get(req, baseino)?;
        Links::create(req, base_ino.clone(), dir, dir.len(), old_inode.clone())
    }

    pub fn unlink(req: &mut Request, path: &str, is_dir: bool) -> Result<(), Error> {
        log!(
            crate::LOG_DEF,
            "dirs::unlink(path={}, is_dir={})",
            path,
            is_dir
        );

        let (base, dir) = {
            let (base_slice, dir_slice) = crate::util::get_base_dir(path);
            (&path[base_slice], &path[dir_slice])
        };

        let parino = Dirs::search(req, base, false)?;
        let parinode = INodes::get(req, parino)?;

        let res = Links::remove(req, parinode.clone(), dir, dir.len(), is_dir);
        if is_dir && res.is_ok() {
            //decrement link count for parent inode by one
            parinode.inode().links -= 1;
        }

        res
    }
}
