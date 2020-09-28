M³
==

This is the official repository of M³: **m**icrokernel-based syste**m** for heterogeneous **m**anycores [1, 2]. M³ is the operating system for a new system architecture that considers heterogeneous compute units (general-purpose cores with different instruction sets, DSPs, FPGAs, fixed-function accelerators, etc.) from the beginning instead of as an afterthought. The goal is to integrate all compute units (CUs) as *first-class citizens*, enabling 1) isolation and secure communication between all types of CUs, 2) direct interactions of all CUs to remove the conventional CPU from the critical path, 3) access to OS services such as file systems and network stacks for all CUs, and 4) context switching support on all CUs.

The system architecture is based on a hardware/operating system co-design with two key ideas:

1) introduce a new hardware component next to each CU used by the OS as the CUs' common interface and
2) let the OS kernel control applications remotely from a different CU.

The new hardware component is called trusted communication unit (TCU). Since not all CUs can be expected to offer the architectural features that are required to run an OS kernel, M³ runs the kernel on a dedicated CU and the  applications on the remaining CUs. To control an application, a kernel controls its TCU remotely, because CU-external resources (other CUs, memories, etc.) can only be accessed via the TCU.

Supported Platforms:
--------------------

Currently, M³ runs on the following platforms:

- gem5, by adding a TCU model to gem5.
- Linux, by using Linux' primitives to simulate the behavior of the TCU and the envisioned system architecture.

Getting Started:
----------------

### Initial setup

If you setup the project on a new (ubuntu) machine make sure to have at least the following packages installed

    $ sudo apt update
    $ sudo apt install git build-essential scons zlib1g-dev \
        m4 libboost-all-dev libssl-dev libgmp3-dev libmpfr-dev \
        libmpc-dev libncurses5-dev texinfo ninja-build

### Preparations for gem5:

The submodule in `platform/gem5` needs to be pulled in and built: \
_(__Hint__: you need username/password-authentication. SSH-authentication won't work due to the submodule git urls)_
The submodule in `platform/gem5` needs to be pulled in and built:

    $ git submodule update --init platform/gem5
    $ cd platform/gem5
    $ scons build/X86/gem5.opt build/X86/gem5.debug [-j 4]

Additionally, you need to build a cross compiler for the desired ISA:

    $ cd cross
    $ ./build.sh (x86_64|arm|riscv)

The cross compiler will be installed to ``<m3-root>/build/cross-<ISA>``.

### Rust

M³ is partially written in Rust and therefore you need to install Rust before building M³. Since M³ still uses some nightly features of Rust, you need the nightly version as follows:

    $ rustup install nightly-2020-09-15
    $ rustup default nightly-2020-09-15
    $ rustup component add rust-src

### Building:

Before you build M³, you should choose your target platform and the build-mode by exporting the corresponding environment variables. For example:

    $ export M3_BUILD=release M3_TARGET=gem5

Now, M³ can be built by using the script `b`:

    $ ./b

### Running:

On all platforms, scenarios can be run by starting the desired boot script in the directory `boot`, e.g.:

    $ ./b run boot/hello.cfg

Note that this command ensures that everything is up to date as well. For more information, run

    $ ./b -h

References:
-----------

[1] Nils Asmussen, Michael Roitzsch, and Hermann Härtig. *M3x: Autonomous Accelerators via Context-Enabled Fast-Path Communication*. To appear in the Proceedings of the 2019 USENIX Annual Technical Conference (USENIX ATC'19).

[2] Nils Asmussen, Marcus Völp, Benedikt Nöthen, Hermann Härtig, and Gerhard Fettweis. *M3: A Hardware/Operating-System Co-Design to Tame Heterogeneous Manycores*. In Proceedings of the Twenty-first International Conference on Architectural Support for Programming Languages and Operating Systems (ASPLOS'16), pages 189-203, April 2016.

Troubleshooting:
----------------

- gem5
  - "six not found":
    - `pip3 install --ignore-installed six`
  - "pid_t getpid() was declared 'extern' and later 'static'":
    - on newer versions of e.g. ubuntu (19.10) the declaration of `pid_t getpid()` in `unistd_ext.h` changed; just remove the old declaration in `src/cpu/kvm/timer.cc` at the beginning of the file and build again
