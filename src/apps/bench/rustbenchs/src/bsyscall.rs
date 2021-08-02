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

use m3::cell::StaticCell;
use m3::cfg;
use m3::com::{MemGate, Perm, RecvGate};
use m3::goff;
use m3::kif;
use m3::math;
use m3::pes::{VPEArgs, PE, VPE};
use m3::profile;
use m3::rc::Rc;
use m3::syscalls;
use m3::test;
use m3::{println, wv_assert_ok, wv_perf, wv_run_test};

static SEL: StaticCell<kif::CapSel> = StaticCell::new(0);

pub fn run(t: &mut dyn test::WvTester) {
    SEL.set(VPE::cur().alloc_sel());

    wv_run_test!(t, noop);
    wv_run_test!(t, activate);
    wv_run_test!(t, create_mgate);
    wv_run_test!(t, create_rgate);
    wv_run_test!(t, create_sgate);
    wv_run_test!(t, create_map);
    wv_run_test!(t, create_srv);
    wv_run_test!(t, derive_mem);
    wv_run_test!(t, exchange);
    wv_run_test!(t, revoke_mem_gate);
    wv_run_test!(t, revoke_recv_gate);
    wv_run_test!(t, revoke_send_gate);
}

fn noop() {
    let mut prof = profile::Profiler::default();

    wv_perf!(
        "noop",
        prof.run_with_id(
            || {
                wv_assert_ok!(syscalls::noop());
            },
            0x10
        )
    );
}

fn activate() {
    let mgate = wv_assert_ok!(MemGate::new(0x1000, Perm::RW));
    let ep = wv_assert_ok!(VPE::cur().epmng_mut().acquire(0));

    let mut prof = profile::Profiler::default();

    wv_perf!(
        "activate",
        prof.run_with_id(
            || {
                wv_assert_ok!(syscalls::activate(
                    ep.sel(),
                    mgate.sel(),
                    kif::INVALID_SEL,
                    0
                ));
            },
            0x11
        )
    );

    VPE::cur().epmng_mut().release(ep, true);
}

fn create_mgate() {
    let mut prof = profile::Profiler::default().repeats(100).warmup(10);

    #[derive(Default)]
    struct Tester(usize);

    impl profile::Runner for Tester {
        fn run(&mut self) {
            wv_assert_ok!(syscalls::create_mgate(
                *SEL,
                VPE::cur().sel(),
                self.0 as goff,
                cfg::PAGE_SIZE as goff,
                Perm::R
            ));
        }

        fn post(&mut self) {
            wv_assert_ok!(syscalls::revoke(
                VPE::cur().sel(),
                kif::CapRngDesc::new(kif::CapType::OBJECT, *SEL, 1),
                true
            ));
        }
    }

    let addr: usize = math::round_dn(&create_mgate as *const _ as usize, cfg::PAGE_SIZE);
    wv_perf!("create_mgate", prof.runner_with_id(&mut Tester(addr), 0x12));
}

fn create_rgate() {
    let mut prof = profile::Profiler::default().repeats(100).warmup(10);

    #[derive(Default)]
    struct Tester();

    impl profile::Runner for Tester {
        fn run(&mut self) {
            wv_assert_ok!(syscalls::create_rgate(*SEL, 10, 10));
        }

        fn post(&mut self) {
            wv_assert_ok!(syscalls::revoke(
                VPE::cur().sel(),
                kif::CapRngDesc::new(kif::CapType::OBJECT, *SEL, 1),
                true
            ));
        }
    }

    wv_perf!(
        "create_rgate",
        prof.runner_with_id(&mut Tester::default(), 0x12)
    );
}

fn create_sgate() {
    let mut prof = profile::Profiler::default().repeats(100).warmup(10);

    #[derive(Default)]
    struct Tester(Option<RecvGate>);

    impl profile::Runner for Tester {
        fn pre(&mut self) {
            if self.0.is_none() {
                self.0 = Some(wv_assert_ok!(RecvGate::new(10, 10)));
            }
        }

        fn run(&mut self) {
            wv_assert_ok!(syscalls::create_sgate(
                *SEL,
                self.0.as_ref().unwrap().sel(),
                0x1234,
                1024
            ));
        }

        fn post(&mut self) {
            wv_assert_ok!(syscalls::revoke(
                VPE::cur().sel(),
                kif::CapRngDesc::new(kif::CapType::OBJECT, *SEL, 1),
                true
            ));
        }
    }

    wv_perf!(
        "create_sgate",
        prof.runner_with_id(&mut Tester::default(), 0x13)
    );
}

fn create_map() {
    if !VPE::cur().pe_desc().has_virtmem() {
        println!("PE has no virtual memory support; skipping");
        return;
    }

    const DEST: kif::CapSel = 0x3000_0000 >> cfg::PAGE_BITS;
    let mut prof = profile::Profiler::default().repeats(100).warmup(10);

    struct Tester(MemGate);

    impl profile::Runner for Tester {
        fn pre(&mut self) {
            // one warmup run, because the revoke leads to an unmap, which flushes and invalidates
            // all cache lines
            wv_assert_ok!(syscalls::create_map(
                DEST,
                VPE::cur().sel(),
                self.0.sel(),
                0,
                1,
                Perm::RW
            ));
        }

        fn run(&mut self) {
            wv_assert_ok!(syscalls::create_map(
                DEST + 1,
                VPE::cur().sel(),
                self.0.sel(),
                1,
                1,
                Perm::RW
            ));
        }

        fn post(&mut self) {
            wv_assert_ok!(syscalls::revoke(
                VPE::cur().sel(),
                kif::CapRngDesc::new(kif::CapType::MAPPING, DEST, 2),
                true
            ));
        }
    }

    let mut tester = Tester {
        0: MemGate::new(cfg::PAGE_SIZE * 2, Perm::RW).unwrap(),
    };
    wv_perf!("create_map", prof.runner_with_id(&mut tester, 0x14));
}

fn create_srv() {
    let mut prof = profile::Profiler::default().repeats(100).warmup(10);

    #[derive(Default)]
    struct Tester(Option<RecvGate>);

    impl profile::Runner for Tester {
        fn pre(&mut self) {
            if self.0.is_none() {
                self.0 = Some(wv_assert_ok!(RecvGate::new(10, 10)));
                self.0.as_mut().unwrap().activate().unwrap();
            }
        }

        fn run(&mut self) {
            wv_assert_ok!(syscalls::create_srv(
                *SEL,
                self.0.as_ref().unwrap().sel(),
                "test",
                0
            ));
        }

        fn post(&mut self) {
            wv_assert_ok!(syscalls::revoke(
                VPE::cur().sel(),
                kif::CapRngDesc::new(kif::CapType::OBJECT, *SEL, 1),
                true
            ));
        }
    }

    wv_perf!(
        "create_srv",
        prof.runner_with_id(&mut Tester::default(), 0x15)
    );
}

fn derive_mem() {
    let mut prof = profile::Profiler::default().repeats(100).warmup(10);

    #[derive(Default)]
    struct Tester(Option<MemGate>);

    impl profile::Runner for Tester {
        fn pre(&mut self) {
            if self.0.is_none() {
                self.0 = Some(wv_assert_ok!(MemGate::new(0x1000, Perm::RW)));
            }
        }

        fn run(&mut self) {
            wv_assert_ok!(syscalls::derive_mem(
                VPE::cur().sel(),
                *SEL,
                self.0.as_ref().unwrap().sel(),
                0,
                0x1000,
                Perm::RW
            ));
        }

        fn post(&mut self) {
            wv_assert_ok!(syscalls::revoke(
                VPE::cur().sel(),
                kif::CapRngDesc::new(kif::CapType::OBJECT, *SEL, 1),
                true
            ));
        }
    }

    wv_perf!(
        "derive_mem",
        prof.runner_with_id(&mut Tester::default(), 0x17)
    );
}

fn exchange() {
    let mut prof = profile::Profiler::default().repeats(100).warmup(10);

    struct Tester {
        vpe: Option<VPE>,
        pe: Rc<PE>,
    }

    impl profile::Runner for Tester {
        fn pre(&mut self) {
            if self.vpe.is_none() {
                self.vpe = Some(wv_assert_ok!(VPE::new_with(
                    self.pe.clone(),
                    VPEArgs::new("test")
                )));
            }
        }

        fn run(&mut self) {
            wv_assert_ok!(syscalls::exchange(
                self.vpe.as_ref().unwrap().sel(),
                kif::CapRngDesc::new(kif::CapType::OBJECT, kif::SEL_VPE, 1),
                *SEL,
                false,
            ));
        }

        fn post(&mut self) {
            wv_assert_ok!(syscalls::revoke(
                self.vpe.as_ref().unwrap().sel(),
                kif::CapRngDesc::new(kif::CapType::OBJECT, *SEL, 1),
                true
            ));
        }
    }

    wv_perf!(
        "exchange",
        prof.runner_with_id(
            &mut Tester {
                vpe: None,
                pe: wv_assert_ok!(PE::new("child")),
            },
            0x18
        )
    );
}

fn revoke_mem_gate() {
    let mut prof = profile::Profiler::default().repeats(100).warmup(10);

    let mgate = wv_assert_ok!(MemGate::new(0x1000, Perm::RW));

    struct Tester {
        mgate: MemGate,
        _derived: Option<MemGate>,
    }

    impl profile::Runner for Tester {
        fn pre(&mut self) {
            self._derived = Some(wv_assert_ok!(self.mgate.derive(0, 0x1000, Perm::RW)));
        }

        fn run(&mut self) {
            self._derived = None;
        }
    }

    let mut tester = Tester {
        mgate,
        _derived: None,
    };
    wv_perf!("revoke_mem_gate", prof.runner_with_id(&mut tester, 0x19));
}

fn revoke_recv_gate() {
    let mut prof = profile::Profiler::default().repeats(100).warmup(10);

    #[derive(Default)]
    struct Tester();

    impl profile::Runner for Tester {
        fn pre(&mut self) {
            wv_assert_ok!(syscalls::create_rgate(*SEL, 10, 10));
        }

        fn run(&mut self) {
            wv_assert_ok!(syscalls::revoke(
                VPE::cur().sel(),
                kif::CapRngDesc::new(kif::CapType::OBJECT, *SEL, 1),
                true
            ));
        }
    }

    wv_perf!(
        "revoke_recv_gate",
        prof.runner_with_id(&mut Tester::default(), 0x20)
    );
}

fn revoke_send_gate() {
    let mut prof = profile::Profiler::default().repeats(100).warmup(10);

    #[derive(Default)]
    struct Tester(Option<RecvGate>);

    impl profile::Runner for Tester {
        fn pre(&mut self) {
            self.0 = Some(wv_assert_ok!(RecvGate::new(10, 10)));
            wv_assert_ok!(syscalls::create_sgate(
                *SEL,
                self.0.as_ref().unwrap().sel(),
                0x1234,
                1024
            ));
        }

        fn run(&mut self) {
            wv_assert_ok!(syscalls::revoke(
                VPE::cur().sel(),
                kif::CapRngDesc::new(kif::CapType::OBJECT, *SEL, 1),
                true
            ));
        }
    }

    wv_perf!(
        "revoke_send_gate",
        prof.runner_with_id(&mut Tester::default(), 0x21)
    );
}
