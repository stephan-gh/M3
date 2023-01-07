/*
 * Copyright (C) 2023 Nils Asmussen, Barkhausen Institut
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

use m3::boxed::Box;
use m3::com::{MemGate, RGateArgs, RecvGate};
use m3::errors::{Code, VerboseError};
use m3::kif::Perm;
use m3::test::{DefaultWvTester, WvTester};
use m3::tiles::{ActivityArgs, ChildActivity, RunningActivity, RunningDeviceActivity, Tile};
use m3::{wv_assert_eq, wv_assert_ok, wv_assert_some, wv_run_test};

use resmng::childs::{Child, ChildManager, OwnChild};
use resmng::config::Domain;
use resmng::requests::Requests;
use resmng::res::Resources;
use resmng::subsys::{ChildStarter, Subsystem, SubsystemBuilder};
use resmng::tiles::TileUsage;

pub fn run(t: &mut dyn WvTester) {
    wv_run_test!(t, subsys_builder);
    wv_run_test!(t, start);
}

fn subsys_builder(t: &mut dyn WvTester) {
    let tile = wv_assert_ok!(Tile::get("clone|own"));
    let mut child = wv_assert_ok!(ChildActivity::new_with(
        tile,
        // ensure that the selector for the resmng and the subsystem don't collide
        ActivityArgs::new("test").first_sel(1000)
    ));

    let (_our_sub, mut res) = wv_assert_ok!(Subsystem::new());

    let mut child_sub = SubsystemBuilder::default();

    wv_assert_ok!(child_sub.add_config("<app args=\"test\"/>", |size| MemGate::new(size, Perm::RW)));
    child_sub.add_mod(wv_assert_ok!(MemGate::new(0x1000, Perm::RW)), "test");
    child_sub.add_mem(wv_assert_ok!(MemGate::new(0x4000, Perm::R)), false);
    child_sub.add_tile(wv_assert_ok!(Tile::get("clone")));

    wv_assert_ok!(child_sub.finalize_async(&mut res, 0, &mut child));

    let run = wv_assert_ok!(child.run(|| {
        let mut t = DefaultWvTester::default();
        let (child_sub, _res) = wv_assert_ok!(Subsystem::new());

        wv_assert_eq!(t, child_sub.mods().len(), 2);
        wv_assert_eq!(t, child_sub.mods()[0].name(), "boot.xml");
        wv_assert_eq!(t, child_sub.mods()[1].name(), "test");

        wv_assert_eq!(t, child_sub.mems().len(), 1);
        wv_assert_eq!(t, child_sub.mems()[0].size(), 0x4000);
        wv_assert_eq!(t, child_sub.mems()[0].reserved(), false);

        wv_assert_eq!(t, child_sub.tiles().len(), 1);

        Ok(())
    }));

    wv_assert_eq!(t, run.wait(), Ok(Code::Success));
}

fn start(t: &mut dyn WvTester) {
    let tile = wv_assert_ok!(Tile::get("clone|own"));
    let mut child = wv_assert_ok!(ChildActivity::new_with(
        tile.clone(),
        ActivityArgs::new("test").first_sel(1000)
    ));

    let (_our_sub, mut res) = wv_assert_ok!(Subsystem::new());
    let mut child_sub = SubsystemBuilder::default();

    wv_assert_ok!(child_sub.add_config(
        "<app args=\"resmngtest\">
             <dom>
                 <app args=\"/bin/rusthello\"/>
             </dom>
         </app>",
        |size| MemGate::new(size, Perm::RW)
    ));
    let tile_quota = wv_assert_ok!(tile.quota());
    child_sub.add_tile(wv_assert_ok!(tile.derive(
        Some(tile_quota.endpoints().remaining() / 2),
        Some(tile_quota.time().remaining() / 2),
        Some(tile_quota.page_tables().remaining() / 2)
    )));

    wv_assert_ok!(child_sub.finalize_async(&mut res, 0, &mut child));

    let run = wv_assert_ok!(child.run(|| {
        let mut t = DefaultWvTester::default();

        let req_rgate = wv_assert_ok!(RecvGate::new_with(
            RGateArgs::default().order(6).msg_order(6),
        ));
        let reqs = Requests::new(req_rgate);

        let mut childs = ChildManager::default();

        let (child_sub, mut res) = wv_assert_ok!(Subsystem::new());

        struct TestStarter {}
        impl ChildStarter for TestStarter {
            fn start(
                &mut self,
                _reqs: &Requests,
                _res: &mut Resources,
                child: &mut OwnChild,
            ) -> Result<(), VerboseError> {
                let act = wv_assert_ok!(ChildActivity::new(
                    child.child_tile().unwrap().tile_obj().clone(),
                    child.name(),
                ));

                let run = RunningDeviceActivity::new(act);
                child.set_running(Box::new(run));

                Ok(())
            }

            fn configure_tile(
                &mut self,
                _res: &mut Resources,
                _tile: &TileUsage,
                _dom: &Domain,
            ) -> Result<(), VerboseError> {
                Ok(())
            }
        }

        let cid = childs.next_id();
        let delayed =
            wv_assert_ok!(child_sub.start(&mut childs, &reqs, &mut res, &mut TestStarter {}));
        wv_assert_eq!(t, delayed.len(), 0);

        wv_assert_eq!(t, childs.children(), 1);
        wv_assert_eq!(t, childs.daemons(), 0);
        wv_assert_eq!(t, childs.foreigns(), 0);

        let child = wv_assert_some!(childs.child_by_id(cid));

        childs.kill_child_async(&reqs, &mut res, child.activity_sel(), Code::Success);

        wv_assert_eq!(t, childs.children(), 0);

        Ok(())
    }));

    wv_assert_eq!(t, run.wait(), Ok(Code::Success));
}
