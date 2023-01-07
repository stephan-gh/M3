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
use m3::errors::{Code, Error, VerboseError};
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
    wv_run_test!(t, start_simple);
    wv_run_test!(t, start_service_deps);
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

fn run_subsys(t: &mut dyn WvTester, cfg: &str, func: fn() -> Result<(), Error>) {
    let tile = wv_assert_ok!(Tile::get("clone|own"));
    let mut child = wv_assert_ok!(ChildActivity::new_with(
        tile.clone(),
        ActivityArgs::new("test").first_sel(1000)
    ));

    let (_our_sub, mut res) = wv_assert_ok!(Subsystem::new());
    let mut child_sub = SubsystemBuilder::default();

    wv_assert_ok!(child_sub.add_config(cfg, |size| MemGate::new(size, Perm::RW)));
    let tile_quota = wv_assert_ok!(tile.quota());
    child_sub.add_tile(wv_assert_ok!(tile.derive(
        Some(tile_quota.endpoints().remaining() / 2),
        Some(tile_quota.time().remaining() / 2),
        Some(tile_quota.page_tables().remaining() / 2)
    )));

    wv_assert_ok!(child_sub.finalize_async(&mut res, 0, &mut child));

    let run = wv_assert_ok!(child.run(func));

    wv_assert_eq!(t, run.wait(), Ok(Code::Success));
}

fn setup_resmng() -> (Requests, ChildManager, Subsystem, Resources) {
    let req_rgate = wv_assert_ok!(RecvGate::new_with(
        RGateArgs::default().order(6).msg_order(6),
    ));
    let reqs = Requests::new(req_rgate);

    let childs = ChildManager::default();

    let (child_sub, res) = wv_assert_ok!(Subsystem::new());

    (reqs, childs, child_sub, res)
}

fn start_simple(t: &mut dyn WvTester) {
    run_subsys(
        t,
        "<app args=\"resmngtest\">
             <dom>
                 <app args=\"/bin/rusthello\"/>
             </dom>
         </app>",
        || {
            let mut t = DefaultWvTester::default();

            let (reqs, mut childs, child_sub, mut res) = setup_resmng();

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
        },
    );
}

fn start_service_deps(t: &mut dyn WvTester) {
    run_subsys(
        t,
        "<app args=\"resmngtest\">
             <dom>
                 <app args=\"1\">
                    <serv name=\"serv\"/>
                 </app>
                 <app args=\"2\">
                    <sess name=\"serv\"/>
                 </app>
                 <app args=\"3\">
                 </app>
                 <app args=\"4\">
                    <sess name=\"serv\" dep=\"false\"/>
                 </app>
             </dom>
         </app>",
        || {
            let mut t = DefaultWvTester::default();

            let (reqs, mut childs, child_sub, mut res) = setup_resmng();

            let cid = childs.next_id();
            let delayed =
                wv_assert_ok!(child_sub.start(&mut childs, &reqs, &mut res, &mut TestStarter {}));
            wv_assert_eq!(t, delayed.len(), 1);
            wv_assert_eq!(t, delayed[0].name(), "2");
            wv_assert_eq!(t, delayed[0].has_unmet_reqs(&mut res), true);

            wv_assert_eq!(t, childs.children(), 3);
            wv_assert_eq!(t, childs.daemons(), 0);
            wv_assert_eq!(t, childs.foreigns(), 0);

            let c1 = wv_assert_some!(childs.child_by_id(cid + 0));
            wv_assert_eq!(t, c1.name(), "1");
            childs.kill_child_async(&reqs, &mut res, c1.activity_sel(), Code::Success);

            wv_assert_eq!(t, childs.children(), 2);

            let c2 = wv_assert_some!(childs.child_by_id(cid + 2));
            wv_assert_eq!(t, c2.name(), "3");
            childs.kill_child_async(&reqs, &mut res, c2.activity_sel(), Code::Success);

            wv_assert_eq!(t, childs.children(), 1);

            let c3 = wv_assert_some!(childs.child_by_id(cid + 3));
            wv_assert_eq!(t, c3.name(), "4");
            childs.kill_child_async(&reqs, &mut res, c3.activity_sel(), Code::Success);

            wv_assert_eq!(t, childs.children(), 0);

            Ok(())
        },
    );
}
