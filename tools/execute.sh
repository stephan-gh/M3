#!/bin/bash

usage() {
    echo "Usage: $1 <crossname> <script> [--debug=<prog>]" 1>&2
    exit 1
}

if [ "$1" = "-h" ] || [ "$1" = "--help" ] || [ "$1" = "-?" ]; then
    usage "$0"
fi

build=build/$M3_TARGET-$M3_ISA-$M3_BUILD
bindir=$build/bin
crossdir="./build/cross-$M3_ISA/host/bin"

if [ $# -lt 2 ]; then
    usage "$0"
fi
crossname="$1"
script=$2
shift 2

debug=""
for p in "$@"; do
    case $p in
        --debug=*)
            debug=${p#--debug=}
            ;;
    esac
done

M3_LOG=${M3_LOG:-Info,Error}
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

generate_m3lx_deps() {
    initrds=$(xmllint --xpath './/dom[@initrd]/@initrd' "$1" 2>/dev/null | wc -l)
    if [ "$initrds" -gt 1 ]; then
        echo "Multiple domains with initrd are not supported" >&2 && exit 1
    fi
    if [ "$initrds" -eq 0 ]; then
        return
    fi

    # generate final initrd
    crossroot="$(readlink -f "$crossdir/../../")"
    initrd="$crossroot/images/rootfs.cpio"
    targetdir="$crossroot/build/buildroot-fs/cpio/target"
    # we build upon the initrd generation of buildroot
    if [ ! -f "$crossroot/build/buildroot-fs/cpio/fakeroot" ]; then
        echo "Please run ./b mkrootfs first" >&2 && exit 1
    fi
    rsync -auH --exclude=/THIS_IS_NOT_YOUR_ROOT_FILESYSTEM "$crossroot/target/" "$targetdir"
    # copy our overlay directory to the target directory (binaries in stripped form)
    for f in "$build"/lxbin/*; do
        "$crossdir/${crossname}strip" -o "$targetdir/$(basename "$f")" "$f"
    done
    cp -a src/m3lx/rootfs/* "$targetdir"
    # now generate image
    ( cd cross/buildroot && PATH="$crossroot/host/sbin:$PATH" FAKEROOTDONTTRYCHOWN=1 \
        "$crossroot/host/bin/fakeroot" -- "$crossroot/build/buildroot-fs/cpio/fakeroot" ) >/dev/null
    rm -rf "$targetdir"

    # determine initrd size
    initrd_size=$(stat --printf="%s" "$initrd")
    # round up to page size
    initrd_size=$(python -c "print('{}'.format(($initrd_size + 0xFFF) & 0xFFFFF000))")
    # ensure that we find it during module lookup
    cp -f "$initrd" "$M3_MOD_PATH/rootfs.cpio"
    cp -f "$build/../riscv-pk/bbl" "$M3_MOD_PATH/bbl"

    # determine memory size for the multiplexer
    mem_size=$(xmllint --xpath 'string(.//dom[@initrd]/@muxmem)' "$1")
    case "$mem_size" in
        *G) mem_size=$(("${mem_size%G*}" * 1024 * 1024 * 1024)) ;;
        *M) mem_size=$(("${mem_size%M*}" * 1024 * 1024)) ;;
        *K) mem_size=$(("${mem_size%K*}" * 1024)) ;;
    esac
    # ensure that it's a power of two. otherwise we can't configure RISC-V's PMP properly
    if [ "$(python -c "print('{}'.format(($mem_size & ($mem_size - 1) == 0)))")" != "True" ]; then
        echo "The memory size ($mem_size) for Linux needs to be a power of two!" >&2 && exit 1
    fi

    # we always place the initrd at the end of the memory region
    mem_off=0x10000000
    initrd_end=$(printf "%#x" $(("$mem_off" + "$mem_size")))
    initrd_start=$(printf "%#x" $((initrd_end - initrd_size)))
    sed -e "s/linux,initrd-start = <.*>;/linux,initrd-start = <$initrd_start>;/g" \
        -e "s/linux,initrd-end = <.*>;/linux,initrd-end = <$initrd_end>;/g" \
        -e "s/reg = <MEM_REGION>;/reg = <0x00000000 $mem_off 0x00000000 $(printf "%#x" "$mem_size")>;/g" \
        "src/m3lx/configs/$M3_TARGET.dts" > "$M3_OUT/m3lx.dts" || exit 1

    # generate dtb
    dtc -O dtb "$M3_OUT/m3lx.dts" -o "$M3_MOD_PATH/m3lx.dtb"
}

get_kernel() {
    gawk 'match($0, /<kernel\s.*args="(.*?)"/, m) {
        printf("%s/%s,", "'"$bindir"'", m[1])
    }' < "$1"
}

get_mods() {
    echo -n "boot.xml=$M3_OUT/boot.xml"

    # extract binaries we need to pass as boot modules
    for name in $(xmllint --xpath ".//app[@args]/@args" "$1" 2>/dev/null | gawk '
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
    for mod in $(xmllint --xpath "/config/mods/mod" "$1" 2>/dev/null | gawk '
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
    generate_m3lx_deps "$1" || exit 1

    kernels=$(get_kernel "$1")
    mods="$(get_mods "$1" "gem5"),tilemux=$bindir/tilemux" || exit 1

    if [ "$M3_GEM5_LOG" = "" ]; then
        M3_GEM5_LOG="Tcu"
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
        cmd="$cmd,"
        c=$((c + 1))
    done

    M3_GEM5_CPUFREQ=${M3_GEM5_CPUFREQ:-1GHz}
    M3_GEM5_MEMFREQ=${M3_GEM5_MEMFREQ:-333MHz}
    M3_GEM5_CFG=${M3_GEM5_CFG:-config/default.py}
    export M3_GEM5_TILES=$M3_CORES
    export M3_GEM5_IDE_DRIVE=$M3_HDD

    params=()
    params=("${params[@]}" --outdir="$M3_OUT" --debug-file=gem5.log --debug-flags="$M3_GEM5_LOG")
    if [ "$M3_GEM5_PAUSE" != "" ]; then
        params=("${params[@]}" --listener-mode=on)
    fi
    if [ "$M3_GEM5_LOGSTART" != "" ]; then
        params=("${params[@]}" --debug-start="$M3_GEM5_LOGSTART")
    fi
    params=("${params[@]}" "$M3_GEM5_CFG" --cpu-type "$M3_GEM5_CPU" --isa "$M3_ISA")
    params=("${params[@]}" --cmd "$cmd" --mods "$mods" --logflags "$M3_LOG")
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
            "$build/toolsbin/ignoreint" platform/gem5/build/$gem5build/gem5.opt "${params[@]}"
        else
            platform/gem5/build/$gem5build/gem5.opt "${params[@]}"
        fi
    fi
}

build_params_hw() {
    generate_config "$1" "$M3_OUT" || exit 1
    generate_m3lx_deps "$1" || exit 1

    kernels=$(get_kernel "$1")
    mods="$(get_mods "$1" "hw"),tilemux=$bindir/tilemux" || exit 1

    if [ "$M3_TARGET" = "hw22" ]; then
        args="--version 0"
    else
        args="--version 2"
    fi
    args="$args --logflags $M3_LOG"

    if [ "$M3_HW_RESET" = "1" ]; then
        args="$args --reset"
    fi
    if [ -n "$M3_HW_TIMEOUT" ]; then
        args="$args --timeout=$M3_HW_TIMEOUT"
    fi
    if [ "$M3_HW_VM" != "0" ]; then
        args="$args --vm"
    fi

    files=("$M3_OUT/boot.xml")
    IFS=','
    c=0
    for karg in $kernels; do
        args="$args --tile '$(basename "$karg")'"
        files=("${files[@]}" "${karg%% *}")
        c=$((c + 1))
    done
    for mod in $mods; do
        args="$args --mod '$mod'"
        files=("${files[@]}" "${mod#*=}")
    done
    unset IFS

    if [ "$M3_HW_M3LX" != "" ]; then
        if [ "$M3_HW_TTY" = "" ]; then
            echo "Please define M3_HW_TTY first." >&2 && exit 1
        fi
        args="$args --serial $M3_HW_TTY"
    fi

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
