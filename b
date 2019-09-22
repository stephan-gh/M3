#!/bin/bash

# jobs
if [ -f /proc/cpuinfo ]; then
    cpus=`cat /proc/cpuinfo | grep '^processor[[:space:]]*:' | wc -l`
else
    cpus=1
fi

# fall back to reasonable defaults
if [ -z $M3_BUILD ]; then
    M3_BUILD='release'
fi
if [ -z $M3_TARGET ]; then
    M3_TARGET='host'
fi
if [ -z $M3_GEM5_OUT ]; then
    M3_GEM5_OUT="run"
fi

if [ "$M3_TARGET" = "gem5" ]; then
    if [ "$M3_ISA" != "arm" ]; then
        M3_ISA='x86_64'
    fi
else
    M3_ISA=`uname -m`
    if [ "$M3_ISA" = "armv7l" ]; then
        M3_ISA="arm"
    fi
fi

export M3_BUILD M3_TARGET M3_ISA

export LD_LIBRARY_PATH="$LD_LIBRARY_PATH:$build/bin"
crossprefix=''
if [ "$M3_TARGET" = "gem5" ]; then
    if [ "$M3_ISA" = "arm" ]; then
        crossprefix="./build/cross-arm/bin/arm-none-eabi-"
    else
        crossprefix="./build/cross-x86_64/bin/x86_64-elf-m3-"
    fi
fi
if [ "$M3_TARGET" = "gem5" ] && [ "$M3_ISA" = "arm" ]; then
    rustabi='gnueabihf'
else
    rustabi='gnu'
fi

# rust env vars
export RUST_TARGET=$M3_ISA-unknown-$M3_TARGET-$rustabi
export RUST_TARGET_PATH=`readlink -f src/toolchain/rust`
export CARGO_TARGET_DIR=`readlink -f build/rust`
export XBUILD_SYSROOT_PATH=$CARGO_TARGET_DIR/sysroot

build=build/$M3_TARGET-$M3_ISA-$M3_BUILD
bindir=$build/bin/

help() {
    echo "Usage: $1 [<cmd> <arg>] [-s] [--no-build|-n]"
    echo ""
    echo "This is a convenience script that is responsible for building everything"
    echo "and running the specified command afterwards. The most important environment"
    echo "variables that influence its behaviour are M3_TARGET=(host|gem5),"
    echo "M3_ISA=(x86_64|arm) [on gem5 only], and M3_BUILD=(debug|release)."
    echo "You can also prevent the script from building everything by specifying -n or"
    echo "--no-build. In this case, only the specified command is executed."
    echo "To build sequentially, i.e. with a single thread, use -s."
    echo ""
    echo "The following commands are available:"
    echo "    clean:                   do a clean in M3"
    echo "    distclean:               remove all build-dirs"
    echo "    run <script>:            run the specified <script>. See directory boot."
    echo "    runq <script>:           run the specified <script> quietly."
    echo "    runvalgrind <script>:    run the specified script in valgrind."
    echo "    clippy:                  run clippy for rust code."
    echo "    doc:                     generate rust documentation."
    echo "    fmt:                     run rustfmt for rust code."
    echo "    macros=<path>:           expand rust macros for app in <path>."
    echo "    dbg=<prog> <script>:     run <script> and debug <prog> in gdb"
    echo "    dis=<prog>:              run objdump -SC <prog> (the cross-compiler version)"
    echo "    elf=<prog>:              run readelf -a <prog> (the cc version)"
    echo "    nms=<prog>:              run nm -SC --size-sort <prog> (the cc version)"
    echo "    nma=<prog>:              run nm -SCn <prog> (the cc version)"
    echo "    straddr=<prog> <string>  search for <string> in <prog>"
    echo "    ctors=<prog>:            show the constructors of <prog>"
    echo "    trace=<progs>:           shows an annotated instruction trace stdin. <progs>"
    echo "                             are the binary names for the symbols. stdin expects"
    echo "                             the gem5.log with Exec,ExecPC enabled."
    echo "    flamegraph=<progs>:      produces a flamegraph with stdin to stdout. <progs>"
    echo "                             are the binary names for the symbols. stdin expects"
    echo "                             the gem5.log with Exec,ExecPC,DtuConnector enabled."
    echo "    snapshot=<progs> <time>: prints the stacktrace of all programs at timestamp"
    echo "                             <time>. <progs> are the binary names for the symbols."
    echo "                             stdin expects the gem5.log with Exec,ExecPC enabled."
    echo "    mkfs=<fsimg> <dir> ...:  create m3-fs in <fsimg> with content of <dir>"
    echo "    shfs=<fsimg> ...:        show m3-fs in <fsimg>"
    echo "    fsck=<fsimg> ...:        run m3fsck on <fsimg>"
    echo "    exfs=<fsimg> <dir>:      export contents of <fsimg> to <dir>"
    echo "    bt=<prog>:               print the backtrace, using given symbols"
    echo "    list:                    list the link-address of all programs"
    echo ""
    echo "Environment variables:"
    echo "    M3_TARGET:               the target. Either 'host' for using the Linux-based"
    echo "                             coarse-grained simulator, or 'gem5'. The default is"
    echo "                             'host'."
    echo "    M3_ISA:                  the ISA to use. On gem5, 'arm' and 'x86_64' is"
    echo "                             supported. On other targets, it is ignored."
    echo "    M3_BUILD:                the build-type. Either debug or release. In debug"
    echo "                             mode optimizations are disabled, debug infos are"
    echo "                             available and assertions are active. In release"
    echo "                             mode all that is disabled. The default is release."
    echo "    M3_VERBOSE:              print executed commands in detail during build."
    echo "    M3_VALGRIND:             for runvalgrind: pass arguments to valgrind."
    echo "    M3_CORES:                # of cores to simulate."
    echo "    M3_FS:                   The filesystem to use (filename only)."
    echo "    M3_HDD:                  The hard drive image to use (filename only)."
    echo "    M3_FSBPE:                The blocks per extent (0 = unlimited)."
    echo "    M3_FSBLKS:               The fs block count (default=16384)."
    echo "    M3_GEM5_DBG:             The trace-flags for gem5 (--debug-flags)."
    echo "    M3_GEM5_DBGSTART:        When to start tracing for gem5 (--debug-start)."
    echo "    M3_GEM5_CPU:             The CPU model (detailed by default)."
    echo "    M3_GEM5_CC:              Enable cache coherence (off by default)."
    echo "    M3_GEM5_OUT:             The output directory of gem5 ('run' by default)."
    echo "    M3_GEM5_DTUPOS:          The DTU position (0=before L1, 1=behind L1 or"
    echo "                             2=behind L2)."
    echo "    M3_GEM5_MMU:             Make use of the core-internal MMU (1 or 0)."
    echo "    M3_GEM5_CPUFREQ:         The CPU frequency (1GHz by default)."
    echo "    M3_GEM5_MEMFREQ:         The memory frequency (333MHz by default)."
    echo "    M3_GEM5_FSNUM:           The number of times to load the FS image."
    echo "    M3_GEM5_PAUSE:           Pause the PE with given number until GDB connects"
    echo "                             (only on gem5 and with command dbg=)."
    exit 0
}

# parse arguments
dobuild=true
cmd=""
script=""
while [ $# -gt 0 ]; do
    case "$1" in
        -h|-\?|--help)
            help $0
            ;;

        -n|--no-build)
            dobuild=false
            ;;

        -s)
            cpus=1
            ;;

        *)
            if [ "$cmd" = "" ]; then
                cmd="$1"
            elif [ "$script" = "" ]; then
                script="$1"
            else
                break
            fi
            ;;
    esac
    shift
done

# for clean and distclean, it makes no sense to build it (this might even fail because e.g. scons has
# a non existing dependency which might be the reason the user wants to do a clean)
if [ "$cmd" = "clean" ] || [ "$cmd" = "distclean" ]; then
    dobuild=false
fi

if $dobuild; then
    echo "Building for $M3_TARGET-$M3_ISA-$M3_BUILD using $cpus jobs..."

    scons -j$cpus
    if [ $? -ne 0 ]; then
        exit 1
    fi
fi

mkdir -p run

run_on_host() {
    echo -n > run/log.txt
    tail -f run/log.txt &
    tailpid=$!
    trap 'stty sane && kill $tailpid' INT
    ./src/tools/execute.sh $1
    kill $tailpid
}

kill_m3_procs() {
    # kill all processes that are using the m3 sockets
    lsof -a -U -u $USER | grep '@m3_ep_' | awk '{ print $2 }' | sort | uniq | xargs kill || true
}

childpids() {
    n=0
    for pid in $(ps h -o pid --ppid $1); do
        if [ $n -gt 0 ]; then
            echo -n ","
        fi
        echo -n $pid
        list=$(childpids $pid)
        if [ "$list" != "" ]; then
            echo -n ","$list
        fi
        n=$(($n + 1))
    done
}

findprog() {
    pids=$(childpids $1)
    if [ "$pids" != "" ]; then
        ps hww -o pid,cmd -p $pids | grep "^\s*[[:digit:]]* [^ ]*$2\b"
    fi
}

# run the specified command, if any
case "$cmd" in
    clean)
        scons -c
        ;;

    distclean)
        rm -Rf build/*
        ;;

    run)
        if [ "$M3_TARGET" = "host" ]; then
            run_on_host $script
            kill_m3_procs 2>/dev/null
        else
            if [ "$DBG_GEM5" = "1" ]; then
                ./src/tools/execute.sh $script
            else
                ./src/tools/execute.sh $script 2>&1 | tee $M3_GEM5_OUT/log.txt
            fi
        fi
        ;;

    runq)
        if [ "$M3_TARGET" = "host" ]; then
            ./src/tools/execute.sh $script
            kill_m3_procs 2>/dev/null
        else
            ./src/tools/execute.sh ./$script >/dev/null
        fi
        ;;

    runvalgrind)
        if [ "$M3_TARGET" = "host" ]; then
            export M3_VALGRIND=${M3_VALGRIND:-"--leak-check=full"}
            run_on_host $script
            kill_m3_procs 2>/dev/null
        else
            echo "Not supported"
        fi
        ;;

    clippy)
        export SYSROOT=$XBUILD_SYSROOT_PATH
        ( cd src && cargo clippy --target $RUST_TARGET -- -A clippy::identity_op )
        ;;

    doc)
        export RUSTFLAGS="--sysroot $XBUILD_SYSROOT_PATH"
        export RUSTDOCFLAGS=$RUSTFLAGS
        for lib in rustm3 rustthread rustresmng; do
            ( cd src/libs/$lib && cargo doc --target $RUST_TARGET )
        done
        ;;

    fmt)
        for f in $(find src -mindepth 2 -name Cargo.toml); do
            echo "Formatting $(dirname $f)..."
            rustfmt $(dirname $f)/src/*.rs
        done
        ;;

    macros=*)
        export RUSTFLAGS="--sysroot $XBUILD_SYSROOT_PATH"
        ( cd ${cmd#macros=} && \
            cargo rustc --target $RUST_TARGET --profile=check \
                -- -Zunstable-options --pretty=expanded | less )
        ;;

    dbg=*)
        if [ "$M3_TARGET" = "host" ]; then
            # does not work in release mode
            if [ "$M3_BUILD" != "debug" ]; then
                echo "Only supported with M3_BUILD=debug."
                exit 1
            fi

            # ensure that we can ptrace non-child-processes
            if [ "`cat /proc/sys/kernel/yama/ptrace_scope`" = "1" ]; then
                echo 0 | sudo tee /proc/sys/kernel/yama/ptrace_scope
            fi

            prog="${cmd#dbg=}"
            M3_WAIT="$prog" ./src/tools/execute.sh $script --debug=${cmd#dbg=} &

            pid=`pgrep -x kernel`
            while [ "$pid" = "" ]; do
                sleep 1
                pid=`pgrep -x kernel`
            done
            if [ "$prog" != "kernel" ]; then
                line=$(findprog $pid $prog)
                while [ "$line" = "" ]; do
                    sleep 1
                    line=$(findprog $pid $prog)
                done
                pid=$(findprog $pid $prog | xargs | cut -d ' ' -f 1)
            fi

            tmp=`mktemp`
            echo "display/i \$pc" >> $tmp
            echo "b main" >> $tmp
            echo "set var wait_for_debugger = 0" >> $tmp
            rust-gdb --tui $build/bin/$prog $pid --command=$tmp

            kill_m3_procs 2>/dev/null
            rm $tmp
        elif [ "$M3_TARGET" = "gem5" ]; then
            truncate --size 0 $M3_GEM5_OUT/log.txt
            ./src/tools/execute.sh $script --debug=${cmd#dbg=} 1>$M3_GEM5_OUT/log.txt 2>&1 &

            # wait until it has started
            while [ "`grep --text "Global frequency set at" $M3_GEM5_OUT/log.txt`" = "" ]; do
                sleep 1
            done

            if [ "$M3_GEM5_PAUSE" != "" ]; then
                port=$(($M3_GEM5_PAUSE + 7000))
            else
                echo "Warning: M3_GEM5_PAUSE not specified; gem5 won't wait for GDB."
                pe=`grep --text "^PE.*$build/bin/${cmd#dbg=}" $M3_GEM5_OUT/log.txt | cut -d : -f 1`
                port=$((${pe#PE} + 7000))
            fi

            gdbcmd=`mktemp`
            echo "target remote localhost:$port" > $gdbcmd
            echo "display/i \$pc" >> $gdbcmd
            echo "b main" >> $gdbcmd
            RUST_GDB=${crossprefix}gdb rust-gdb --tui $bindir/${cmd#dbg=} --command=$gdbcmd

            killall -9 gem5.opt
            rm $gdbcmd
        else
            echo "Not supported"
        fi
        ;;

    dis=*)
        ${crossprefix}objdump -SC $bindir/${cmd#dis=} | less
        ;;

    elf=*)
        ${crossprefix}readelf -aW $bindir/${cmd#elf=} | c++filt | less
        ;;

    nms=*)
        ${crossprefix}nm -SC --size-sort $bindir/${cmd#nms=} | less
        ;;

    nma=*)
        ${crossprefix}nm -SCn $bindir/${cmd#nma=} | less
        ;;

    straddr=*)
        binary=$bindir/${cmd#straddr=}
        str=$script
        echo "Strings containing '$str' in $binary:"
        # find base address of .rodata
        base=`${crossprefix}readelf -S $binary | grep .rodata | \
            xargs | cut -d ' ' -f 5`
        # find section number of .rodata
        section=`${crossprefix}readelf -S $binary | grep .rodata | \
            sed -e 's/.*\[\s*\([[:digit:]]*\)\].*/\1/g'`
        # grep for matching lines, prepare for better use of awk and finally add offset to base
        ${crossprefix}readelf -p $section $binary | grep $str | \
            sed 's/^ *\[ *\([[:xdigit:]]*\)\] *\(.*\)$/0x\1 \2/' | \
            awk '{ printf("0x%x: %s %s %s %s %s %s\n",0x'$base' + strtonum($1),$2,$3,$4,$5,$6,$7) }'
        ;;

    ctors=*)
        file=$bindir/${cmd#ctors=}
        rdelf=${crossprefix}readelf
        pat=".ctors\|.init_array"
        if [ "$M3_ISA" = "x86_64" ]; then
            off=0x`$rdelf -S "$file" | grep $pat | sed -e 's/\[.*\]//g' | xargs | cut -d ' ' -f 4`
            len=0x`$rdelf -S "$file" | grep $pat -A1 | grep '^       ' | xargs | cut -d ' ' -f 1`
            bytes=8
        else
            section=`$rdelf -S "$file" | grep $pat | sed -e 's/\[.*\]//g' | xargs`
            echo $section
            off=0x`echo "$section" | cut -d ' ' -f 4`
            len=0x`echo "$section" | cut -d ' ' -f 5`
            bytes=4
        fi
        echo "Constructors in $file ($off : $len):"
        if [ "$off" != "0x" ]; then
            od -t x$bytes "$file" -j $off -N $len -v -w$bytes | grep ' ' | while read line; do
                addr=${line#* }
                ${crossprefix}nm -C -l "$file" | grep -m 1 $addr
            done
        fi
        ;;

    trace=*)
        paths=""
        for f in $(echo ${cmd#trace=} | sed "s/,/ /g"); do
            paths="$paths $build/bin/$f"
        done
        $build/tools/gem5log $M3_ISA trace $paths | less
        ;;

    flamegraph=*)
        paths=""
        for f in $(echo ${cmd#flamegraph=} | sed "s/,/ /g"); do
            paths="$paths $build/bin/$f"
        done
        # inferno-flamegraph is available at https://github.com/jonhoo/inferno
        $build/tools/gem5log $M3_ISA flamegraph $paths | inferno-flamegraph --countname ns
        ;;

    snapshot=*)
        paths=""
        for f in $(echo ${cmd#snapshot=} | sed "s/,/ /g"); do
            paths="$paths $build/bin/$f"
        done
        $build/tools/gem5log $M3_ISA snapshot $script $paths
        ;;

    mkfs=*)
        if [[ "$@" = "" ]]; then
            $build/tools/mkm3fs $build/${cmd#mkfs=} $script src/tests 8192 256 95
        else
            $build/tools/mkm3fs $build/${cmd#mkfs=} $script $@
        fi
        ;;

    shfs=*)
        $build/tools/shm3fs $build/${cmd#shfs=} $script $@
        ;;

    fsck=*)
        $build/tools/m3fsck $build/${cmd#fsck=} $script
        ;;

    exfs=*)
        $build/tools/exm3fs $build/${cmd#exfs=} $script
        ;;

    bt=*)
        ./src/tools/backtrace.py "$crossprefix" $bindir/${cmd#bt=}
        ;;

    list)
        echo "Start of section .text:"
        ls -1 $build/bin | grep -v '\.\(o\|a\)$' | while read l; do
            ${crossprefix}readelf -S $build/bin/$l | \
                grep " \.text " | awk "{ printf(\"%20s: %s\n\",\"$l\",\$5) }"
        done
        ;;
esac
