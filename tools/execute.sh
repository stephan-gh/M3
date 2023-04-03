#!/bin/bash

usage() {
    echo "Usage: $1 <script> [--debug=<prog>]" 1>&2
    exit 1
}

if [ "$1" = "-h" ] || [ "$1" = "--help" ] || [ "$1" = "-?" ]; then
    usage "$0"
fi

build=build/$M3_TARGET-$M3_ISA-$M3_BUILD
bindir=$build/bin

if [ $# -lt 1 ]; then
    usage "$0"
fi
script=$1
shift

debug=""
for p in "$@"; do
    case $p in
        --debug=*)
            debug=${p#--debug=}
            ;;
    esac
done

M3_MOD_PATH=${M3_MOD_PATH:-$build}

generate_config() {
    if [ ! -f "$1" ]; then
        echo "error: '$1' is not a file" >&2 && exit 1
    fi

    # validate config
    xmllint --schema misc/boot.xsd --noout "$1" > /dev/null || exit 1

    # extract env variables and set them
    env=$(xmllint --xpath "/config/env/text()" "$1" 2>/dev/null)
    if [ "$env" != "" ]; then
        for e in $env; do
            var=${e%%=*}
            val=${e#*=}
            old_env=$(env | grep "^$var=")
            old_val=${old_env#*=}
            if [ "$old_val" != "" ]; then
                # warn if the user has set it to a different value
                if [ "$old_val" != "$val" ]; then
                    echo -n "Warning: $var is already set to '$old_val',"
                    echo " ignoring overwrite to '$val' by '$1'."
                fi
            else
                # only set it if the user has not already set the environment variable
                export "${e?}"
            fi
        done
    fi

    # extract runtime part; this can fail if there is no app element (e.g., standalone.xml)
    xmllint --xpath /config/dom/app "$1" 2>/dev/null > "$2/boot.xml" || true
}

get_mods() {
    echo -n "boot.xml=$M3_OUT/boot.xml"

    # extract binaries we need to pass as boot modules
    for name in $(xmllint --xpath ".//app[@args]/@args" "$1" 2>/dev/null | awk -e '
        # we currently assume that binaries starting with "/" are loaded from the FS
        match($0, /args="([^/][^[:space:]]*).*"/, m) {
            print(m[1])
        }
    '); do
        # use the stripped binary from the default fs on hw to save time during loading
        if [ "$2" = "hw" ]; then
            if [ -f "$build/src/fs/default/bin/$name" ]; then
                path="$build/src/fs/default/bin/$name"
            else
                path="$build/src/fs/default/sbin/$name"
            fi
        else
            if [ "$name" = "disk" ] && [ "$M3_HDD" = "" ]; then
                echo "Please specify the HDD image to use via M3_HDD." >&2 && exit 1
            fi
            path="$bindir/$name"
        fi
        if [ ! -f "$path" ]; then
            echo "Binary '$path' does not exist." >&2 && exit 1
        fi
        echo -n ",$name=$path"
    done

    # add additional boot modules from config
    for mod in $(xmllint --xpath "/config/mods/mod" "$1" 2>/dev/null | awk -e '
        match($0, /<mod\s+name="(.*?)"\s+file="(.*?)"/, m) {
            printf("%s=%s\n", m[1], m[2])
        }
    ')
    do
        name=${mod%%=*}
        path=${mod#*=}
        if [ ! -f "$M3_MOD_PATH/$path" ]; then
            echo "Boot module '$M3_MOD_PATH/$path' does not exist." >&2 && exit 1
        fi
        echo -n ",$name=$M3_MOD_PATH/$path"
    done
}

build_params_gem5() {
    generate_config "$1" "$M3_OUT" || exit 1

    kernels=$(perl -ne 'printf("'"$bindir"/'%s,", $1) if /<kernel\s.*args="(.*?)"/' < "$1")
    mods=$(get_mods "$1" "gem5") || exit 1

    if [ "$M3_GEM5_DBG" = "" ]; then
        M3_GEM5_DBG="Tcu"
    fi
    if [ "$M3_GEM5_CPU" = "" ]; then
        if [ "$debug" != "" ]; then
            M3_GEM5_CPU="TimingSimpleCPU"
        else
            M3_GEM5_CPU="DerivO3CPU"
        fi
    fi
    if [ "$M3_HDD" != "" ] && [ ! -f "$M3_HDD" ]; then
        echo "Hard disk image '$M3_HDD' does not exist." >&2 && exit 1
    fi

    M3_CORES=${M3_CORES:-16}

    cmd=$kernels
    c=$(echo -n "$cmd" | sed 's/[^,]//g' | wc -c)
    while [ "$c" -lt "$M3_CORES" ]; do
        cmd="$cmd$bindir/tilemux,"
        c=$((c + 1))
    done

    M3_GEM5_CPUFREQ=${M3_GEM5_CPUFREQ:-1GHz}
    M3_GEM5_MEMFREQ=${M3_GEM5_MEMFREQ:-333MHz}
    M3_GEM5_CFG=${M3_GEM5_CFG:-config/default.py}
    export M3_GEM5_TILES=$M3_CORES
    export M3_GEM5_IDE_DRIVE=$M3_HDD

    params=()
    params=("${params[@]}" --outdir="$M3_OUT" --debug-file=gem5.log --debug-flags="$M3_GEM5_DBG")
    if [ "$M3_GEM5_PAUSE" != "" ]; then
        params=("${params[@]}" --listener-mode=on)
    fi
    if [ "$M3_GEM5_DBGSTART" != "" ]; then
        params=("${params[@]}" --debug-start="$M3_GEM5_DBGSTART")
    fi
    params=("${params[@]}" "$M3_GEM5_CFG" --cpu-type "$M3_GEM5_CPU" --isa "$M3_ISA")
    params=("${params[@]}" --cmd "$cmd" --mods "$mods")
    params=("${params[@]}" --cpu-clock="$M3_GEM5_CPUFREQ" --sys-clock="$M3_GEM5_MEMFREQ")
    if [ "$M3_GEM5_PAUSE" != "" ]; then
        params=("${params[@]}" --pausetile="$M3_GEM5_PAUSE")
    fi

    if [ "$M3_ISA" = "x86_64" ]; then
        gem5build="X86"
    elif [ "$M3_ISA" = "arm" ]; then
        gem5build="ARM"
    elif [ "$M3_ISA" = "riscv" ]; then
        gem5build="RISCV"
    else
        echo "Unsupported ISA: $M3_ISA" >&2
        exit 1
    fi

    # remove all coverage files
    rm -rf "$M3_OUT"/coverage-*-*.profraw

    export M5_PATH=$build
    if [ "$DBG_GEM5" != "" ]; then
        tmp=$(mktemp)
        trap 'rm -f $tmp' EXIT ERR INT TERM
        {
            echo "b main"
            echo -n "run" "${params[@]}"
            echo
        } > "$tmp"
        gdb --tui platform/gem5/build/$gem5build/gem5.debug "--command=$tmp"
    else
        if [ "$debug" != "" ]; then
            "$build/tools/ignoreint" platform/gem5/build/$gem5build/gem5.opt "${params[@]}"
        else
            platform/gem5/build/$gem5build/gem5.opt "${params[@]}"
        fi
    fi
}

build_params_hw() {
    generate_config "$1" "$M3_OUT" || exit 1

    kernels=$(perl -ne 'printf("%s,", $1) if /<kernel\s.*args="(.*?)"/' < "$1")
    mods=$(get_mods "$1" "hw") || exit 1

    if [ "$M3_TARGET" = "hw22" ]; then
        args="--version 0"
    else
        args="--version 1"
    fi

    if [ "$M3_HW_RESET" = "1" ]; then
        args="$args --reset"
    fi
    if [ -n "$M3_HW_TIMEOUT" ]; then
        args="$args --timeout=$M3_HW_TIMEOUT"
    fi
    if [ "$M3_HW_VM" != "0" ]; then
        args="$args --vm"
    fi

    files=("$M3_OUT/boot.xml" "$bindir/tilemux")
    IFS=','
    c=0
    for karg in $kernels; do
        args="$args --tile '$karg'"
        files=("${files[@]}" "$bindir/${karg%% *}")
        c=$((c + 1))
    done
    for mod in $mods; do
        args="$args --mod '$mod'"
        files=("${files[@]}" "${mod#*=}")
    done
    while [ $c -lt 8 ]; do
        args="$args --tile tilemux"
        c=$((c + 1))
    done
    unset IFS

    fpga="--fpga $M3_HW_FPGA_NO"

    {
        echo "#!/bin/sh"
        echo "export PYTHONPATH=\$HOME/$M3_HW_FPGA_DIR/python:\$PYTHONPATH"
        # echo "export RUST_FILE_LOG=debug"
        echo ""
        if [ "$debug" != "" ]; then
            # start everything
            echo 'echo -n > .running'
            echo 'trap "rm -f .running 2>/dev/null" SIGINT SIGTERM EXIT'
            echo 'rm -f .ready'
            echo "python3 ./fpga.py $fpga $args --debug $M3_HW_PAUSE &>log.txt &"
            # wait until it's finished or failed
            echo 'fpga=$!'
            echo 'echo "Waiting until FPGA has been initialized..."'
            # shellcheck disable=SC2016
            echo 'while [ "`cat .ready 2>/dev/null`" = "" ] && [ -f /proc/$fpga/cmdline ]; do sleep 1; done'
            # stop if it failed
            echo "[ -f /proc/\$fpga/cmdline ] || { cat log.txt && exit 1; }"
            # make sure we clean up everything
            echo 'trap "trap - SIGTERM && kill -- -$$" SIGINT SIGTERM EXIT'
            # start openocd
            echo "OPENOCD=\$HOME/tcu/fpga_tools/debug"
            echo "\$OPENOCD/openocd -f \$OPENOCD/fpga_switch.cfg >openocd.log 2>&1"

            # make sure that openocd is stopped
            trap 'ssh -t $M3_HW_FPGA_HOST "killall openocd"' ERR INT TERM
        else
            echo "python3 ./fpga.py $fpga $args 2>&1 | tee -i log.txt"
        fi
    } > "$M3_OUT/run.sh"

    rsync -rz \
        tools/fpga.py platform/hw/fpga_tools/python "${files[@]}" "$M3_OUT/run.sh" \
        "$M3_HW_FPGA_HOST:$M3_HW_FPGA_DIR"

    ssh -t "$M3_HW_FPGA_HOST" "cd $M3_HW_FPGA_DIR && sh run.sh"
    scp "$M3_HW_FPGA_HOST:$M3_HW_FPGA_DIR/log.txt" "$M3_HW_FPGA_HOST:$M3_HW_FPGA_DIR/log/pm*" "$M3_OUT"
}

if [ "$M3_TARGET" = "gem5" ] || [ "$M3_RUN_GEM5" = "1" ]; then
    build_params_gem5 "$script"
elif [ "$M3_TARGET" = "hw" ] || [ "$M3_TARGET" = "hw22" ]; then
    build_params_hw "$script"
else
    echo "Unknown target '$M3_TARGET'"
fi

# ensure that we get into cooked mode again
stty sane
