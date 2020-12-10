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

//! Contains unittest utilities inspired by WvTest (https://github.com/apenwarr/wvtest)

/// Runs the tests
pub trait WvTester {
    /// Runs the given test suite
    fn run_suite(&mut self, name: &str, f: &dyn Fn(&mut dyn WvTester));
    /// Runs the given test
    fn run_test(&mut self, name: &str, file: &str, f: &dyn Fn());
}

/// Convenience macro that calls [`WvTester::run_suite`](::test::WvTester::run_suite) and uses
/// the function name as suite name
#[macro_export]
macro_rules! wv_run_suite {
    ($t:expr, $func:path) => {
        $t.run_suite(stringify!($func), &$func)
    };
}

/// Convenience macro that calls [`WvTester::run_test`](::test::WvTester::run_test) and uses
/// the function name as test name
#[macro_export]
macro_rules! wv_run_test {
    ($t:expr, $func:path) => {
        $t.run_test(stringify!($func), file!(), &$func)
    };
}

/// Convenience macro that runs the given benchmark and reports the result
#[macro_export]
macro_rules! wv_perf {
    ($name:expr, $bench:expr) => {
        ::m3::println!("! {}:{}  PERF \"{}\": {}", file!(), line!(), $name, $bench);
    };
}

extern "C" {
    #[allow(dead_code)]
    fn wvtest_failed();
}

/// Convenience macro that tests whether $a is true and reports failures
#[macro_export]
macro_rules! wv_assert {
    ($a:expr) => {{
        match (&$a) {
            (a_val) => {
                if !*a_val {
                    ::m3::println!("! {}:{}  {:?} FAILED", file!(), line!(), &*a_val);
                    crate::wvtest_failed();
                }
            },
        }
    }};
}

/// Convenience macro that tests whether $a and $b are equal and reports failures
#[macro_export]
macro_rules! wv_assert_eq {
    ($a:expr, $b:expr) => ({
        match (&$a, &$b) {
            (a_val, b_val) => {
                if *a_val != *b_val {
                    ::m3::println!("! {}:{}  {:?} == {:?} FAILED", file!(), line!(), &*a_val, &*b_val);
                    crate::wvtest_failed();
                }
            }
        }
    });

    ($a:expr, $b:expr, $($arg:tt)+) => ({
        match (&$a, &$b) {
            (a_val, b_val) => {
                if *a_val != *b_val {
                    ::m3::println!("! {}:{}  {} FAILED", file!(), line!(), format_args!($($arg)+));
                    crate::wvtest_failed();
                }
            }
        }
    });
}

/// Convenience macro that tests whether the argument is [`Ok`], returns the inner value if so, and
/// panics otherwise
#[macro_export]
macro_rules! wv_assert_ok {
    ($res:expr) => {{
        match $res {
            Ok(r) => r,
            Err(e) => {
                ::m3::println!(
                    "! {}:{}  expected Ok for {}, got {:?} FAILED",
                    file!(),
                    line!(),
                    stringify!($res),
                    e
                );
                panic!("Stopping tests here.")
            },
        }
    }};
}

/// Convenience macro that tests whether the argument is [`Some`], returns the inner value if so,
/// and panics otherwise
#[macro_export]
macro_rules! wv_assert_some {
    ($res:expr) => {{
        match $res {
            Some(r) => r,
            None => {
                ::m3::println!(
                    "! {}:{}  expected Some for {}, received None FAILED",
                    file!(),
                    line!(),
                    stringify!($res)
                );
                panic!("Stopping tests here.")
            },
        }
    }};
}

/// Convenience macro that tests whether the argument is [`Err`] with the given error code
#[macro_export]
macro_rules! wv_assert_err {
    ($res:expr, $err:expr) => {{
        match $res {
            Ok(r) => {
                ::m3::println!("! {}:{}  received okay: {:?} FAILED", file!(), line!(), r);
                crate::wvtest_failed();
            },
            Err(ref e) if e.code() != $err => {
                ::m3::println!(
                    "! {}:{}  received error {:?}, expected {:?} FAILED",
                    file!(),
                    line!(),
                    e,
                    $err
                );
                crate::wvtest_failed();
            },
            Err(_) => {},
        }
    }};
    ($res:expr, $err1:expr, $err2:expr) => {{
        match $res {
            Ok(r) => {
                ::m3::println!("! {}:{}  received okay: {:?} FAILED", file!(), line!(), r);
                crate::wvtest_failed();
            },
            Err(ref e) if e.code() != $err1 && e.code() != $err2 => {
                ::m3::println!(
                    "! {}:{}  received error {:?}, expected {:?} or {:?} FAILED",
                    file!(),
                    line!(),
                    e,
                    $err1,
                    $err2
                );
                crate::wvtest_failed();
            },
            Err(_) => {},
        }
    }};
}
