#!/bin/bash

# fall back to reasonable defaults
if [ -z "$M3_BUILD" ]; then
    M3_BUILD='release'
fi
if [ -z "$M3_TARGET" ]; then
    M3_TARGET='gem5'
fi
if [ -z "$M3_ISA" ]; then
    M3_ISA='riscv'
fi
if [ -z "$M3_OUT" ]; then
    M3_OUT="run"
fi

# set target
if [ "$M3_TARGET" = "gem5" ]; then
    if [ "$M3_ISA" != "arm" ] && [ "$M3_ISA" != "x86_64" ] && [ "$M3_ISA" != "riscv" ]; then
        echo "ISA $M3_ISA not supported for target gem5." >&2 && exit 1
    fi
elif [ "$M3_TARGET" = "hw" ]; then
    M3_ISA="riscv"
else
    echo "Target $M3_TARGET not supported." >&2 && exit 1
fi

if [ "$M3_BUILD" != "debug" ] && [ "$M3_BUILD" != "release" ] && [ "$M3_BUILD" != "coverage" ]; then
    echo "Build mode $M3_BUILD not supported." >&2 && exit 1
fi
if [ "$M3_BUILD" = "coverage" ] && [ "$M3_ISA" != "riscv" ] && [ "$M3_ISA" != "x86_64" ]; then
    echo "Coverage mode is only supported with M3_ISA=riscv and M3_ISA=x86_64." >&2 && exit 1
fi

export M3_BUILD M3_TARGET M3_ISA M3_OUT

# determine cross compiler and rust ABI based on target and ISA
export LD_LIBRARY_PATH="$LD_LIBRARY_PATH:$build/bin"
crossdir="./build/cross-$M3_ISA/bin"
if [ "$M3_ISA" = "arm" ]; then
    crossprefix="$crossdir/arm-none-eabi-"
elif [ "$M3_ISA" = "riscv" ]; then
    crossprefix="$crossdir/riscv64-unknown-elf-"
else
    crossprefix="$crossdir/x86_64-elf-m3-"
fi
export PATH=$crossdir:$PATH
if [ "$M3_TARGET" = "gem5" ] && [ "$M3_ISA" = "arm" ]; then
    rustabi='musleabi'
elif [ "$M3_BUILD" = "coverage" ]; then
    rustabi='muslcov'
else
    rustabi='musl'
fi

# rust env vars
rusttoolchain=$(readlink -f src/toolchain/rust)
rustbuild=$(readlink -f build/rust)
if [ "$M3_ISA" = "riscv" ]; then
    rustisa="riscv64"
else
    rustisa="$M3_ISA"
fi
export RUST_TARGET=$rustisa-linux-$M3_TARGET-$rustabi
export RUST_TARGET_PATH=$rusttoolchain
export CARGO_TARGET_DIR=$rustbuild
export XBUILD_SYSROOT_PATH=$CARGO_TARGET_DIR/sysroot
rust_args=(--target "$RUST_TARGET" -Z "build-std=core,alloc,std,panic_abort")
# configure TARGET_CFLAGS for llvmprofile within minicov
case "$M3_ISA" in
    x86_64) export TARGET_CFLAGS="-msoft-float -mno-sse" ;;
    riscv)  export TARGET_CFLAGS="-march=rv64imafdc -mabi=lp64" ;;
    arm)    export TARGET_CFLAGS="-march=armv7-a -fshort-enums" ;;
esac

build=build/$M3_TARGET-$M3_ISA-$M3_BUILD
bindir=$build/bin/

help() {
    echo "Usage: $1 [-n] [<cmd> <arg>]"
    echo ""
    echo "This is a convenience script that is responsible for building everything"
    echo "and running the specified command afterwards. The most important environment"
    echo "variables that influence its behaviour are M3_TARGET=(gem5|hw),"
    echo "M3_ISA=(x86_64|arm|riscv) [on gem5 only], and M3_BUILD=(debug|release|coverage)."
    echo ""
    echo "The flag -n skips the build and executes the given command directly. This"
    echo "can be handy if, for example, the build is currently broken."
    echo ""
    echo "The following commands are available:"
    echo "    ninja ...:               run ninja with given arguments."
    echo "    run <script>:            run the specified <script>. See directory boot."
    echo "    rungem5 <script>:        run the specified <script> on gem5. See directory boot."
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
    echo "    hwitrace=<progs>:        shows an annotated hardware instruction trace. <progs>"
    echo "                             are the binary names for the symbols. stdin expects"
    echo "                             the run/pm*-instrs.log."
    echo "    trace=<progs>:           shows an annotated instruction trace. <progs> are"
    echo "                             the binary names for the symbols. stdin expects the"
    echo "                             gem5.log with Exec,ExecPC enabled."
    echo "    flamegraph=<progs>:      produces a flamegraph with stdin to stdout. <progs>"
    echo "                             are the binary names for the symbols. stdin expects"
    echo "                             the gem5.log with Exec,ExecPC,TcuConnector enabled."
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
    echo "    M3_TARGET:               the target. Either 'gem5' or 'hw', default is 'gem5'."
    echo "    M3_ISA:                  the ISA to use. On gem5, 'arm', 'riscv', and 'x86_64'"
    echo "                             is supported. On other targets, it is ignored."
    echo "    M3_BUILD:                the build-type. Either debug or release. In debug mode"
    echo "                             optimizations are disabled, debug infos are available,"
    echo "                             and assertions are active. In release mode all that is"
    echo "                             disabled. The default is release."
    echo "    M3_REM_HOST:             if set, the build is performed on this host in"
    echo "                             M3_REM_DIR. All source files are synced to the remote"
    echo "                             host before the build and the build files are synced"
    echo "                             back afterwards."
    echo "    M3_REM_DIR:              the directory in which the remote build takes place."
    echo "    M3_VERBOSE:              print executed commands in detail during build."
    echo "    M3_CORES:                # of cores to simulate."
    echo "    M3_FS:                   The filesystem to use (filename only)."
    echo "    M3_HDD:                  The hard drive image to use (filename only)."
    echo "    M3_OUT:                  The output directory ('run' by default)."
    echo "    M3_GEM5_DBG:             The trace-flags for gem5 (--debug-flags)."
    echo "    M3_GEM5_DBGSTART:        When to start tracing for gem5 (--debug-start)."
    echo "    M3_GEM5_CPU:             The CPU model (detailed by default)."
    echo "    M3_GEM5_CPUFREQ:         The CPU frequency (1GHz by default)."
    echo "    M3_GEM5_MEMFREQ:         The memory frequency (333MHz by default)."
    echo "    M3_GEM5_FSNUM:           The number of times to load the FS image."
    echo "    M3_GEM5_PAUSE:           Pause the tile with given id until GDB connects"
    echo "                             (only on gem5 and with command dbg=). Numbers"
    echo "                             are translated into C0T<number>, but ids can also"
    echo "                             be specified in the form of 'C<chip>T<tile>'."
    echo "    M3_HW_SSH:               The SSH alias for the FPGA PC (default: syn)"
    echo "    M3_HW_FPGA:              The FPGA number (default 0 = IP 192.168.42.240)"
    echo "    M3_HW_RESET:             Reset the FPGA before starting"
    echo "    M3_HW_VM:                Use virtual memory (default = 1)"
    echo "    M3_HW_TIMEOUT:           Stop execution after given number of seconds."
    echo "    M3_HW_PAUSE:             Pause the tile with given number at startup"
    echo "                             (only on hw and with command dbg=)."
    exit 0
}

# parse arguments
case "$1" in
    -h|-\?|--help)
        help "$0"
        ;;
esac

skipbuild=0
cmd=""
script=""
while [ $# -gt 0 ]; do
    if [ "$1" = "-n" ]; then
        skipbuild=1
    elif [ "$cmd" = "" ]; then
        cmd="$1"
    elif [ "$script" = "" ]; then
        script="$1"
    else
        break
    fi
    shift
done

mkdir -p $build $M3_OUT

ninjaargs=()
if [ "$M3_VERBOSE" != "" ]; then
    ninjaargs=("${ninjaargs[@]}" -v)
fi

if [ $skipbuild -eq 0 ]; then
    filesid=$build/.all-files.id
    find src -type f > $filesid.new
    # redo the configuration if any file was added/removed
    if [ ! -f $build/build.ninja ] || ! cmp $filesid.new $filesid &>/dev/null; then
        echo "Configuring for $M3_TARGET-$M3_ISA-$M3_BUILD..." >&2
        ./configure.py || exit 1
        mv $filesid.new $filesid
    fi
fi

case "$cmd" in
    ninja)
        ninja -f $build/build.ninja "${ninjaargs[@]}" "$script" "$@"
        exit $?
        ;;
esac

if [ $skipbuild -eq 0 ]; then
    if [ "$M3_REM_HOST" != "" ]; then
        echo "Building for $M3_TARGET-$M3_ISA-$M3_BUILD remotely at $M3_REM_HOST:$M3_REM_DIR..." >&2
        # sync all sources to the remote host and check whether anything was transferred
        if [ "$(rsync -az . "--exclude=/.ninja*" --exclude=/platform --exclude=/build \
                    --stats "$M3_REM_HOST:$M3_REM_DIR" |
                grep "Number of regular files transferred: 0")" = "" ]; then
            # if there was something transferred, build it on the remote host. source the .profile
            # to set environment variables (e.g. PATH to include ~/.cargo/bin).
            if ssh "$M3_REM_HOST" \
                   "source .profile && " \
                   "cd $M3_REM_DIR && " \
                   "M3_BUILD=$M3_BUILD M3_TARGET=$M3_TARGET M3_ISA=$M3_ISA ./b"; then
                # and transfer build files back
                rsync -az \
                    "$M3_REM_HOST:$M3_REM_DIR/build/$M3_TARGET-$M3_ISA-$M3_BUILD/" \
                    "build/$M3_TARGET-$M3_ISA-$M3_BUILD/"
            else
                # store the current date to some file to ensure that we transfer something next time
                # we try to build, regardless of whether something changed.
                date --rfc-3339=ns > .remote-build-failed
                exit 1
            fi
        fi
    else
        echo "Building for $M3_TARGET-$M3_ISA-$M3_BUILD..." >&2
        ninja -f $build/build.ninja "${ninjaargs[@]}" >&2 || {
            # ensure that we regenerate the build.ninja next time. Since ninja does not accept the
            # build.ninja, it will also not detect changes our build files in order to regenerate it.
            # Therefore, force ourself to regenerate it by removing our "files id".
            rm -f $filesid
            exit 1
        }
    fi
fi

# run the specified command, if any
case "$cmd" in
    run)
        if [ "$DBG_GEM5" = "1" ]; then
            ./src/tools/execute.sh "$script"
        else
            ./src/tools/execute.sh "$script" 2>&1 | tee $M3_OUT/log.txt
        fi
        ;;

    rungem5)
        M3_RUN_GEM5=1 ./src/tools/execute.sh "$script" 2>&1 | tee $M3_OUT/log.txt
        ;;

    clippy)
        while IFS= read -r -d '' f; do
            # vmtest only works on RISC-V
            if [ "$M3_ISA" != "riscv" ] && [[ $f =~ "vmtest" ]]; then
                continue;
            fi
            # gem5log+hwitrace are always built for the host OS (not our host target)
            target=()
            if [[ ! $f =~ "gem5log" ]] && [[ ! $f =~ "hwitrace" ]] && [[ ! $f =~ "netdbg" ]]; then
                target=("${target[@]}" "${rust_args[@]}")
            fi
            echo "Running clippy for $(dirname "$f")..."
            ( cd "$(dirname "$f")" && cargo clippy "${target[@]}" -- \
                -A clippy::identity_op \
                -A clippy::manual_range_contains \
                -A clippy::assertions_on_constants \
                -A clippy::upper_case_acronyms \
                -A clippy::empty_loop )
        done < <(find src -mindepth 2 -name Cargo.toml -print0)
        ;;

    doc)
        export RUSTFLAGS="--sysroot $XBUILD_SYSROOT_PATH"
        export RUSTDOCFLAGS=$RUSTFLAGS
        for lib in src/libs/rust/*; do
            if [ -d "$lib" ]; then
                ( cd "$lib" && cargo doc "${rust_args[@]}" )
            fi
        done
        ;;

    fmt)
        while IFS= read -r -d '' f; do
            if [[ "$f" =~ "src/libs/musl" ]] && [[ ! "$f" =~ "src/libs/musl/m3" ]]; then
                continue
            fi
            if [[ "$f" =~ "src/libs/flac" ]] || [[ "$f" =~ "src/apps/bsdutils" ]] ||
                [[ "$f" =~ "src/libs/leveldb" ]] || [[ "$f" =~ "src/libs/llvmprofile" ]] ||
                [[ "$f" =~ "src/libs/axieth" ]]; then
                continue
            fi

            echo "Formatting $f..."
            clang-format -i "$f"
        done < <(find src \( -name "*.cc" -or -name "*.h" \) -print0)

        while IFS= read -r -d '' f; do
            echo "Formatting $(dirname "$f")..."
            rustfmt "$(dirname "$f")"/src/*.rs
        done < <(find src -mindepth 2 -name Cargo.toml -print0)
        ;;

    macros=*)
        export RUSTFLAGS="--sysroot $XBUILD_SYSROOT_PATH"
        ( cd "${cmd#macros=}" && \
            cargo rustc "${rust_args[@]}" --profile=check -- -Zunpretty=expanded | less )
        ;;

    dbg=*)
        if [ "$M3_TARGET" = "gem5" ] || [ "$M3_RUN_GEM5" = "1" ]; then
            if [ "$M3_GEM5_PAUSE" = "" ]; then
                echo "Please set M3_GEM5_PAUSE to the tile to debug (e.g., '1' or 'C1T04')."
                exit 1
            fi

            truncate --size 0 $M3_OUT/log.txt
            ./src/tools/execute.sh "$script" "--debug=${cmd#dbg=}" 1>$M3_OUT/log.txt 2>&1 &

            # wait until we know the port
            port=""
            attemps=0
            while [ "$port" = "" ]; do
                if [[ $M3_GEM5_PAUSE =~ C*T* ]]; then
                    tile="$M3_GEM5_PAUSE"
                else
                    tile="C0T$M3_GEM5_PAUSE"
                fi
                port=$(grep --text "$tile.remote_gdb" $M3_OUT/log.txt | cut -d ' ' -f 9)
                if [ "$port" = "" ]; then
                    if [ $attemps -gt 5 ]; then
                        echo "Unable to find port for tile '$tile' after 5 attempts."
                        exit 1
                    fi
                    sleep 1
                fi
                attemps=$((attemps + 1))
            done

            gdbcmd=$(mktemp)
            {
                echo "target remote localhost:$port"
                echo "display/i \$pc"
                echo "b main"
            } > "$gdbcmd"
            RUST_GDB=${crossprefix}gdb rust-gdb --tui "$bindir/${cmd#dbg=}" "--command=$gdbcmd"

            killall -9 gem5.opt
            rm "$gdbcmd"
        else
            if [ "$M3_HW_PAUSE" = "" ]; then
                echo "Please set M3_HW_PAUSE to the tile to debug."
                exit 1
            fi
            ./src/tools/execute.sh "$script" "--debug=${cmd#dbg=}" &>/dev/null &

            port=$((3340 + M3_HW_PAUSE))
            ssh -N -L 30000:localhost:$port "${M3_HW_SSH:-syn}" 2>/dev/null &
            trap 'trap - SIGTERM && kill -- -$$' SIGINT SIGTERM EXIT

            echo -n "Connecting..."
            time=0
            while [ "$(telnet localhost 30000 2>/dev/null | grep '\+')" = "" ]; do
                # after some warmup, detect if something went wrong
                if [ $time -gt 5 ]; then
                    ssh "${M3_HW_SSH:-syn}" "test -e m3/.running" ||
                        { echo "Remote side stopped." && exit 1; }
                fi
                echo -n "."
                sleep 1
                time=$((time + 1))
            done

            gdbcmd=$(mktemp)
            {
                echo "target remote localhost:30000"
                echo "set \$t0 = 0"                  # ensure that we set the default stack pointer
                echo "set \$pc = 0x10000000"         # go to entry point
            } > "$gdbcmd"

            # differentiate between baremetal components and others
            entry=$(${crossprefix}readelf -h "$bindir/${cmd#dbg=}" | \
                grep "Entry point" | awk '{ print($4) }')
            if [ "$entry" = "0x10000000" ]; then
                echo "b env_run" >> "$gdbcmd"
                symbols=$bindir/${cmd#dbg=}
            else
                {
                    echo "tb __app_start"
                    echo "c"
                    echo "symbol-file $bindir/${cmd#dbg=}"
                    echo "b main"
                } >> "$gdbcmd"
                symbols=$bindir/tilemux
            fi
            echo "display/i \$pc" >> "$gdbcmd"

            RUST_GDB=${crossprefix}gdb rust-gdb --tui "$symbols" "--command=$gdbcmd"
        fi
        ;;

    dis=*)
        ${crossprefix}objdump -dC "$bindir/${cmd#dis=}" | less
        ;;

    elf=*)
        ${crossprefix}readelf -aW "$bindir/${cmd#elf=}" | c++filt | less
        ;;

    nms=*)
        ${crossprefix}nm -SC --size-sort "$bindir/${cmd#nms=}" | less
        ;;

    nma=*)
        ${crossprefix}nm -SCn "$bindir/${cmd#nma=}" | less
        ;;

    straddr=*)
        binary=$bindir/${cmd#straddr=}
        str=$script
        echo "Strings containing '$str' in $binary:"
        # find base address of .rodata
        base=$(${crossprefix}readelf -S "$binary" | grep .rodata | \
            xargs | cut -d ' ' -f 5)
        # find section number of .rodata
        section=$(${crossprefix}readelf -S "$binary" | grep .rodata | \
            sed -e 's/.*\[\s*\([[:digit:]]*\)\].*/\1/g')
        # grep for matching lines, prepare for better use of awk and finally add offset to base
        ${crossprefix}readelf -p "$section" "$binary" | grep "$str" | \
            sed 's/^ *\[ *\([[:xdigit:]]*\)\] *\(.*\)$/0x\1 \2/' | \
            awk '{ printf("0x%x: %s %s %s %s %s %s\n",0x'"$base"' + strtonum($1),$2,$3,$4,$5,$6,$7) }'
        ;;

    ctors=*)
        file=$bindir/${cmd#ctors=}
        section=$(${crossprefix}readelf -SW "$file" | \
            grep "\.ctors\|\.init_array" | sed -e 's/\[.*\]//g' | xargs)
        off=0x$(echo "$section" | cut -d ' ' -f 4)
        len=0x$(echo "$section" | cut -d ' ' -f 5)
        if [ "$M3_ISA" = "x86_64" ] || [ "$M3_ISA" = "riscv" ]; then
            bytes=8
        else
            bytes=4
        fi
        echo "Constructors in $file ($off : $len):"
        if [ "$off" != "0x" ]; then
            od -t x$bytes "$file" -j "$off" -N "$len" -v -w$bytes | grep ' ' | while read -r line; do
                addr=${line#* }
                ${crossprefix}nm -C -l "$file" 2>/dev/null | grep -m 1 "$addr"
            done
        fi
        ;;

    hwitrace=*)
        paths=()
        names=${cmd#hwitrace=}
        for f in ${names//,/ }; do
            paths=("${paths[@]}" "$build/bin/$f")
        done
        $build/tools/hwitrace $crossprefix "${paths[@]}" | less
        ;;

    trace=*)
        paths=()
        names=${cmd#trace=}
        for f in ${names//,/ }; do
            paths=("${paths[@]}" "$build/bin/$f")
        done
        $build/tools/gem5log $M3_ISA trace "${paths[@]}" | less
        ;;

    flamegraph=*)
        paths=()
        names=${cmd#flamegraph=}
        for f in ${names//,/ }; do
            paths=("${paths[@]}" "$build/bin/$f")
        done
        # inferno-flamegraph is available at https://github.com/jonhoo/inferno
        $build/tools/gem5log $M3_ISA flamegraph "${paths[@]}" | inferno-flamegraph --countname ns
        ;;

    snapshot=*)
        paths=()
        names=${cmd#snapshot=}
        for f in ${names//,/ }; do
            paths=("${paths[@]}" "$build/bin/$f")
        done
        $build/tools/gem5log $M3_ISA snapshot "$script" "${paths[@]}"
        ;;

    mkfs=*)
        $build/tools/mkm3fs "$build/${cmd#mkfs=}" "$script" "$@"
        ;;

    shfs=*)
        $build/tools/shm3fs "$build/${cmd#shfs=}" "$script" "$@"
        ;;

    fsck=*)
        $build/tools/m3fsck "$build/${cmd#fsck=}" "$script"
        ;;

    exfs=*)
        $build/tools/exm3fs "$build/${cmd#exfs=}" "$script"
        ;;

    bt=*)
        ./src/tools/backtrace.py "$crossprefix" "$bindir/${cmd#bt=}"
        ;;

    list)
        echo "Start of section .text:"
        while IFS= read -r -d '' l; do
            ${crossprefix}readelf -S "$build/bin/$l" | \
                grep " \.text " | awk "{ printf(\"%20s: %s\n\",\"$l\",\$5) }"
        done < <(find $build/bin -type f \! \( -name "*.o" -o -name "*.a" \) -printf "%f\0") | sort -k 2
        ;;
esac
