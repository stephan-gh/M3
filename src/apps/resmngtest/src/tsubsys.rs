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

use m3::com::MemGate;
use m3::errors::Code;
use m3::kif::Perm;
use m3::test::{DefaultWvTester, WvTester};
use m3::tiles::{Activity, ActivityArgs, ChildActivity, RunningActivity, Tile};
use m3::time::TimeDuration;
use m3::{wv_assert, wv_assert_eq, wv_assert_ok, wv_assert_some, wv_run_test};

use resmng::childs::Child;
use resmng::subsys::{Subsystem, SubsystemBuilder};

use crate::helper::{run_subsys, setup_resmng, TestStarter};

pub fn run(t: &mut dyn WvTester) {
    wv_run_test!(t, subsys_builder);
    wv_run_test!(t, start_simple);
    wv_run_test!(t, start_service_deps);
    wv_run_test!(t, start_resource_split);
}

fn subsys_builder(t: &mut dyn WvTester) {
    let tile = wv_assert_ok!(Tile::get("compat|own"));
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
    child_sub.add_tile(wv_assert_ok!(Tile::get("compat")));

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

fn start_simple(t: &mut dyn WvTester) {
    run_subsys(
        t,
        "<app args=\"resmngtest\">
             <dom>
                 <app args=\"/bin/rusthello\"/>
             </dom>
         </app>",
        |_subsys| {},
        || {
            let mut t = DefaultWvTester::default();

            let (reqs, mut childs, child_sub, mut res) = setup_resmng();

            let cid = childs.next_id();
            let delayed = wv_assert_ok!(child_sub.start_async(
                &mut childs,
                &reqs,
                &mut res,
                &mut TestStarter {}
            ));
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
             </dom>
             <dom>
                 <app args=\"2\">
                    <sess name=\"serv\"/>
                 </app>
             </dom>
             <dom>
                 <app args=\"3\">
                 </app>
             </dom>
             <dom>
                 <app args=\"4\">
                    <sess name=\"serv\" dep=\"false\"/>
                 </app>
             </dom>
         </app>",
        |_subsys| {},
        || {
            let mut t = DefaultWvTester::default();

            let (reqs, mut childs, child_sub, mut res) = setup_resmng();

            let cid = childs.next_id();
            let delayed = wv_assert_ok!(child_sub.start_async(
                &mut childs,
                &reqs,
                &mut res,
                &mut TestStarter {}
            ));
            wv_assert_eq!(t, delayed.len(), 1);
            wv_assert_eq!(t, delayed[0].name(), "2");
            wv_assert_eq!(t, delayed[0].has_unmet_reqs(&res), true);

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

fn start_resource_split(t: &mut dyn WvTester) {
    if !Activity::own().tile_desc().has_virtmem() {
        m3::println!("Skipping test on tile without VM support");
        return;
    }

    run_subsys(
        t,
        "<app args=\"resmngtest\">
             <dom>
                 <app args=\"1\"/>
             </dom>
             <dom>
                 <app args=\"2\"/>
            </dom>
             <dom>
                 <app args=\"3\" usermem=\"16K\" pagetables=\"24\"/>
                 <app args=\"4\" kernmem=\"2K\" time=\"2ms\"/>
                 <app args=\"5\" eps=\"16\"/>
            </dom>
         </app>",
        |_subsys| {},
        || {
            let mut t = DefaultWvTester::default();

            let (reqs, mut childs, child_sub, mut res) = setup_resmng();

            let cid = childs.next_id();
            let delayed = wv_assert_ok!(child_sub.start_async(
                &mut childs,
                &reqs,
                &mut res,
                &mut TestStarter {}
            ));
            wv_assert_eq!(t, delayed.len(), 0);

            wv_assert_eq!(t, childs.children(), 5);
            wv_assert_eq!(t, childs.daemons(), 0);
            wv_assert_eq!(t, childs.foreigns(), 0);

            let c1 = wv_assert_some!(childs.child_by_id(cid + 0));
            let c2 = wv_assert_some!(childs.child_by_id(cid + 1));
            let c3 = wv_assert_some!(childs.child_by_id(cid + 2));
            let c4 = wv_assert_some!(childs.child_by_id(cid + 3));
            let c5 = wv_assert_some!(childs.child_by_id(cid + 4));

            wv_assert_eq!(t, c1.name(), "1");
            wv_assert_eq!(t, c2.name(), "2");
            wv_assert_eq!(t, c3.name(), "3");
            wv_assert_eq!(t, c4.name(), "4");
            wv_assert_eq!(t, c5.name(), "5");

            // kernel memory
            {
                let c1_kmem = wv_assert_ok!(c1.kmem().unwrap().quota());
                let c2_kmem = wv_assert_ok!(c2.kmem().unwrap().quota());
                let c3_kmem = wv_assert_ok!(c3.kmem().unwrap().quota());
                let c4_kmem = wv_assert_ok!(c4.kmem().unwrap().quota());
                let c5_kmem = wv_assert_ok!(c5.kmem().unwrap().quota());
                // different domains have different kernel memory quotas
                wv_assert!(t, c1_kmem.id() != c2_kmem.id());
                wv_assert!(t, c3_kmem.id() != c2_kmem.id());
                // since c4 has specified the amount, it receives its own kmem quota
                wv_assert!(t, c4_kmem.id() != c2_kmem.id());
                // c3 and c5 share a domain and use the default amount, therefore shared
                wv_assert_eq!(t, c3_kmem.id(), c5_kmem.id());
                wv_assert_eq!(t, c1_kmem.remaining(), c2_kmem.remaining());
                // c4 has specified its amount
                wv_assert_eq!(t, c4_kmem.remaining(), 2 * 1024);
                // since c3 and c5 share a domain, but each application gets the same kmem amount, the
                // kmem quota of c3 and c5 is twice as large as c2's.
                wv_assert_eq!(t, c2_kmem.remaining() * 2, c3_kmem.remaining());
            }

            // user memory
            {
                let c1_umem = c1.mem().pool().borrow();
                let c2_umem = c2.mem().pool().borrow();
                let c3_umem = c3.mem().pool().borrow();
                let c4_umem = c4.mem().pool().borrow();
                let c5_umem = c5.mem().pool().borrow();
                wv_assert!(t, c1_umem.slices()[0].addr() != c2_umem.slices()[0].addr());
                wv_assert!(t, c3_umem.slices()[0].addr() != c2_umem.slices()[0].addr());
                // c3, c4, and c5 share a domain and therefore share the pool
                wv_assert_eq!(t, c3_umem.slices()[0].addr(), c4_umem.slices()[0].addr());
                wv_assert_eq!(t, c3_umem.slices()[0].addr(), c5_umem.slices()[0].addr());
                wv_assert_eq!(t, c3.mem().quota(), 16 * 1024);
                // all except c3 have the same quota
                wv_assert_eq!(t, c1.mem().quota(), c2.mem().quota());
                wv_assert_eq!(t, c1.mem().quota(), c4.mem().quota());
                wv_assert_eq!(t, c1.mem().quota(), c5.mem().quota());
            }

            // endpoints
            {
                let c1_tile = c1.child_tile().unwrap();
                let c2_tile = c2.child_tile().unwrap();
                let c3_tile = c3.child_tile().unwrap();
                let c4_tile = c4.child_tile().unwrap();
                let c5_tile = c5.child_tile().unwrap();
                // check tile sharing
                wv_assert!(t, c1_tile.tile_id() != c2_tile.tile_id());
                wv_assert!(t, c1_tile.tile_id() != c3_tile.tile_id());
                wv_assert_eq!(t, c3_tile.tile_id(), c4_tile.tile_id());
                wv_assert_eq!(t, c3_tile.tile_id(), c5_tile.tile_id());
                // check ep quota sharing
                let c1_quota = *wv_assert_ok!(c1_tile.tile_obj().quota()).endpoints();
                let c2_quota = *wv_assert_ok!(c2_tile.tile_obj().quota()).endpoints();
                let c3_quota = *wv_assert_ok!(c3_tile.tile_obj().quota()).endpoints();
                let c4_quota = *wv_assert_ok!(c4_tile.tile_obj().quota()).endpoints();
                let c5_quota = *wv_assert_ok!(c5_tile.tile_obj().quota()).endpoints();
                wv_assert!(t, c1_quota.id() != c2_quota.id());
                wv_assert!(t, c1_quota.id() != c3_quota.id());
                wv_assert!(t, c3_quota.id() != c5_quota.id());
                wv_assert_eq!(t, c3_quota.id(), c4_quota.id());
                // check ep quotas
                wv_assert_eq!(t, c5_quota.total(), 16);
                wv_assert_eq!(t, c1_quota.total(), c2_quota.total());
                wv_assert_eq!(t, c1_quota.total(), c3_quota.total());
                wv_assert_eq!(t, c1_quota.total(), c4_quota.total());
            }

            // pagetables
            {
                let c1_tile = c1.child_tile().unwrap();
                let c2_tile = c2.child_tile().unwrap();
                let c3_tile = c3.child_tile().unwrap();
                let c4_tile = c4.child_tile().unwrap();
                let c5_tile = c5.child_tile().unwrap();
                // check pagetable quota sharing
                let c1_quota = *wv_assert_ok!(c1_tile.tile_obj().quota()).page_tables();
                let c2_quota = *wv_assert_ok!(c2_tile.tile_obj().quota()).page_tables();
                let c3_quota = *wv_assert_ok!(c3_tile.tile_obj().quota()).page_tables();
                let c4_quota = *wv_assert_ok!(c4_tile.tile_obj().quota()).page_tables();
                let c5_quota = *wv_assert_ok!(c5_tile.tile_obj().quota()).page_tables();
                wv_assert!(t, c1_quota.id() != c2_quota.id());
                wv_assert!(t, c1_quota.id() != c3_quota.id());
                wv_assert!(t, c3_quota.id() != c5_quota.id());
                wv_assert_eq!(t, c4_quota.id(), c5_quota.id());
                // check pagetable quotas
                wv_assert_eq!(t, c3_quota.total(), 24);
                // note that c1 and c4/c5 are not necessarily the same, because we split the
                // available pagetables on each tile among the apps on that tile
                wv_assert_eq!(t, c1_quota.total(), c2_quota.total());
                wv_assert_eq!(t, c4_quota.total(), c5_quota.total());
            }

            // time
            {
                let c1_tile = c1.child_tile().unwrap();
                let c2_tile = c2.child_tile().unwrap();
                let c3_tile = c3.child_tile().unwrap();
                let c4_tile = c4.child_tile().unwrap();
                let c5_tile = c5.child_tile().unwrap();
                // check time quota sharing
                let c1_quota = *wv_assert_ok!(c1_tile.tile_obj().quota()).time();
                let c2_quota = *wv_assert_ok!(c2_tile.tile_obj().quota()).time();
                let c3_quota = *wv_assert_ok!(c3_tile.tile_obj().quota()).time();
                let c4_quota = *wv_assert_ok!(c4_tile.tile_obj().quota()).time();
                let c5_quota = *wv_assert_ok!(c5_tile.tile_obj().quota()).time();
                wv_assert!(t, c1_quota.id() != c2_quota.id());
                wv_assert!(t, c1_quota.id() != c3_quota.id());
                wv_assert!(t, c3_quota.id() != c4_quota.id());
                wv_assert_eq!(t, c3_quota.id(), c5_quota.id());
                // check time quotas (1ms is the default quota)
                wv_assert_eq!(t, c1_quota.total(), TimeDuration::from_millis(1));
                wv_assert_eq!(t, c2_quota.total(), TimeDuration::from_millis(1));
                // c3 and c5 share their quota and have twice the default quota
                wv_assert_eq!(t, c3_quota.total(), TimeDuration::from_millis(2));
                wv_assert_eq!(t, c4_quota.total(), TimeDuration::from_millis(2));
            }

            let c5_sel = c5.activity_sel();
            let c4_sel = c4.activity_sel();
            let c3_sel = c3.activity_sel();
            let c2_sel = c2.activity_sel();
            let c1_sel = c1.activity_sel();
            childs.kill_child_async(&reqs, &mut res, c5_sel, Code::Success);
            childs.kill_child_async(&reqs, &mut res, c4_sel, Code::Success);
            childs.kill_child_async(&reqs, &mut res, c3_sel, Code::Success);
            childs.kill_child_async(&reqs, &mut res, c2_sel, Code::Success);
            childs.kill_child_async(&reqs, &mut res, c1_sel, Code::Success);

            wv_assert_eq!(t, childs.children(), 0);

            Ok(())
        },
    );
}
