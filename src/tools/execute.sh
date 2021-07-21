#!/bin/bash

usage() {
    echo "Usage: $1 <script> [--debug=<prog>]" 1>&2
    exit 1
}

if [ "$1" = "-h" ] || [ "$1" = "--help" ] || [ "$1" = "-?" ]; then
    usage $0
fi

build=build/$M3_TARGET-$M3_ISA-$M3_BUILD
bindir=$build/bin
hwssh=${M3_HW_SSH:-syn}

if [ $# -lt 1 ]; then
    usage $0
fi
script=$1
shift

debug=""
for p in $@; do
    case $p in
        --debug=*)
            debug=${p#--debug=}
            ;;
    esac
done

if [ "$M3_FS" = "" ]; then
    M3_FS="default.img"
fi
export M3_FS

if [ "$M3_HDD" = "" ]; then
    M3_HDD_PATH="build/$M3_TARGET-$M3_ISA-$M3_BUILD/disk.img"
else
    M3_HDD_PATH=$M3_HDD
fi

generate_config() {
    if [ ! -f $1 ]; then
        echo "error: '$1' is not a file" >&2 && exit 1
    fi

    hd=$M3_HDD_PATH
    fs=build/$M3_TARGET-$M3_ISA-$M3_BUILD/$M3_FS
    fssize=`stat --format="%s" $fs`
    sed "
        s#\$fs.path#$fs#g;
        s#\$fs.size#$fssize#g;
        s#\$hd.path#$hd#g;
    " < $1 > $2/boot-all.xml

    xmllint --schema misc/boot.xsd --noout $2/boot-all.xml > /dev/null || exit 1
    # this can fail if there is no app element (e.g., standalone.xml)
    xmllint --xpath /config/dom/app $2/boot-all.xml > $2/boot.xml || true
}

build_params_host() {
    generate_config $1 $M3_OUT || exit 1

    kargs=$(perl -ne '/<kernel\s.*args="(.*?)"/ && print $1' < $M3_OUT/boot-all.xml)
    mods=$(perl -ne 'printf(" '$bindir'/%s", $1) if /app\s.*args="([^\/"\s]+).*"/' < $M3_OUT/boot-all.xml)
    echo "$bindir/$kargs $M3_OUT/boot.xml$mods"
}

build_params_gem5() {
    generate_config $1 $M3_OUT || exit 1

    kargs=$(perl -ne 'printf("'$bindir/'%s,", $1) if /<kernel\s.*args="(.*?)"/' < $M3_OUT/boot-all.xml)
    mods=$(perl -ne 'printf(",'$bindir'/%s", $1) if /app\s.*args="([^\/"\s]+).*"/' < $M3_OUT/boot-all.xml)
    mods="$M3_OUT/boot.xml$mods"

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

    M3_CORES=${M3_CORES:-16}

    cmd=$kargs
    c=$(echo -n $cmd | sed 's/[^,]//g' | wc -c)
    while [ $c -lt $M3_CORES ]; do
        cmd="$cmd$bindir/pemux,"
        c=$((c + 1))
    done

    if [[ $mods == *disk* ]] && [ "$M3_HDD" = "" ]; then
        ./src/tools/disk.py create $M3_HDD_PATH $build/$M3_FS
    fi

    M3_GEM5_CPUFREQ=${M3_GEM5_CPUFREQ:-1GHz}
    M3_GEM5_MEMFREQ=${M3_GEM5_MEMFREQ:-333MHz}
    M3_GEM5_CFG=${M3_GEM5_CFG:-config/default.py}
    export M3_GEM5_PES=$M3_CORES
    export M3_GEM5_FS=$build/$M3_FS
    export M3_GEM5_IDE_DRIVE=$M3_HDD_PATH

    params=$(mktemp)
    trap "rm -f $params" EXIT ERR INT TERM

    echo -n "--outdir=$M3_OUT --debug-file=gem5.log --debug-flags=$M3_GEM5_DBG" >> $params
    if [ "$M3_GEM5_PAUSE" != "" ]; then
        echo -n " --listener-mode=on" >> $params
    fi
    if [ "$M3_GEM5_DBGSTART" != "" ]; then
        echo -n " --debug-start=$M3_GEM5_DBGSTART" >> $params
    fi
    echo -n " $M3_GEM5_CFG --cpu-type $M3_GEM5_CPU --isa $M3_ISA" >> $params
    echo -n " --cmd \"$cmd\" --mods \"$mods\"" >> $params
    echo -n " --cpu-clock=$M3_GEM5_CPUFREQ --sys-clock=$M3_GEM5_MEMFREQ" >> $params
    if [ "$M3_GEM5_PAUSE" != "" ]; then
        echo -n " --pausepe=$M3_GEM5_PAUSE" >> $params
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

    export M5_PATH=$build
    if [ "$DBG_GEM5" != "" ]; then
        tmp=$(mktemp)
        trap "rm -f $tmp" EXIT ERR INT TERM
        echo "b main" >> $tmp
        echo -n "run " >> $tmp
        cat $params >> $tmp
        echo >> $tmp
        gdb --tui platform/gem5/build/$gem5build/gem5.debug --command=$tmp
    else
        if [ "$debug" != "" ]; then
            params="$params $build/tools/ignoreint"
        fi
        xargs -a $params platform/gem5/build/$gem5build/gem5.opt
    fi
}

build_params_hw() {
    generate_config $1 $M3_OUT || exit 1

    kargs=$(perl -ne 'printf("%s;", $1) if /<kernel\s.*args="(.*?)"/' < $M3_OUT/boot-all.xml)
    mods=$(perl -ne 'printf("%s;", $1) if /app\s.*args="([^\/"\s]+).*"/' < $M3_OUT/boot-all.xml)

    args="--mod boot.xml"
    if [ "$M3_HW_RESET" = "1" ]; then
        args="$args --reset"
    fi
    if [ "$M3_HW_VM" = "1" ]; then
        pemux="pemux"
        args="$args --vm"
    else
        pemux="peidle"
    fi

    files="$M3_OUT/boot.xml $bindir/$pemux"
    IFS=';'
    c=0
    for karg in $kargs; do
        args="$args --pe '$karg'"
        files="$files $bindir/"${karg%% *}
        c=$((c + 1))
    done
    for mod in $mods; do
        args="$args --mod '$mod'"
        # use the stripped binary from the default fs
        files="$files $build/src/fs/default/bin/$(basename $mod)"
    done
    while [ $c -lt 8 ]; do
        args="$args --pe $pemux"
        c=$((c + 1))
    done
    unset IFS

    if [ "`grep '$fs' $1`" != "" ]; then
        files="$files $build/$M3_FS"
        args="$args --fs $(basename $build/$M3_FS)"
    fi

    fpga="--fpga ${M3_HW_FPGA:-0}"

    echo -n > $M3_OUT/run.sh
    echo "#!/bin/sh" >> $M3_OUT/run.sh
    echo "export PYTHONPATH=\$HOME/tcu/fpga_tools/python:\$PYTHONPATH" >> $M3_OUT/run.sh
    echo "export PYTHONPATH=\$HOME/tcu/fpga_tools/pyelftools-0.26:\$PYTHONPATH" >> $M3_OUT/run.sh
    # echo "export RUST_FILE_LOG=debug" >> $M3_OUT/run.sh
    echo "" >> $M3_OUT/run.sh
    if [ "$debug" != "" ]; then
        # start everything
        echo 'echo -n > .running' >> $M3_OUT/run.sh
        echo 'trap "rm -f .running 2>/dev/null" SIGINT SIGTERM EXIT' >> $M3_OUT/run.sh
        echo 'rm -f .ready' >> $M3_OUT/run.sh
        echo "python3 ./fpga.py $fpga $args --debug $M3_HW_PAUSE &>log.txt &" >> $M3_OUT/run.sh
        # wait until it's finished or failed
        echo 'fpga=$!' >> $M3_OUT/run.sh
        echo 'echo "Waiting until FPGA has been initialized..."' >> $M3_OUT/run.sh
        echo 'while [ "`cat .ready 2>/dev/null`" = "" ] && [ -f /proc/$fpga/cmdline ]; do sleep 1; done' >> $M3_OUT/run.sh
        # stop if it failed
        echo '[ -f /proc/$fpga/cmdline ] || { cat log.txt && exit 1; }' >> $M3_OUT/run.sh
        # make sure we clean up everything
        echo 'trap "trap - SIGTERM && kill -- -$$" SIGINT SIGTERM EXIT' >> $M3_OUT/run.sh
        # start openocd
        echo 'OPENOCD=$HOME/tcu/fpga_tools/debug' >> $M3_OUT/run.sh
        echo '$OPENOCD/openocd -f $OPENOCD/fpga_switch.cfg >openocd.log 2>&1' >> $M3_OUT/run.sh

        # make sure that openocd is stopped
        trap "ssh -t $hwssh 'killall openocd'" ERR INT TERM
    else
        echo "python3 ./fpga.py $fpga $args 2>&1 | tee -i log.txt" >> $M3_OUT/run.sh
    fi

    rsync -z src/tools/fpga.py $files $M3_OUT/run.sh $hwssh:m3

    ssh -t $hwssh "cd m3 && sh run.sh"
    scp "$hwssh:m3/{log.txt,log/pm*}" $M3_OUT
}

if [ "$M3_TARGET" = "host" ]; then
    # use unique temp directory for this run and remove it afterwards
    dir=$(mktemp -d)
    export M3_HOST_TMP=$dir
    trap "rm -rf $dir" EXIT ERR INT TERM

    set -m

    params=$(build_params_host $script) || exit 1

    if [[ $params == *disk* ]] && [ "$M3_HDD" = "" ]; then
        ./src/tools/disk.py create $M3_HDD_PATH $build/$M3_FS
    fi

    if [ "$M3_VALGRIND" != "" ]; then
        $build/tools/setpgrp valgrind $M3_VALGRIND $params &
    else
        $build/tools/setpgrp setarch $(uname -m) -R $params &
    fi

    # kill the whole process group on exit; just to be sure
    kernelpid=$!
    trap "/bin/kill -- -$kernelpid 2>/dev/null" EXIT ERR INT TERM

    fg
elif [ "$M3_TARGET" = "gem5" ] || [ "$M3_RUN_GEM5" = "1" ]; then
    build_params_gem5 $script
elif [ "$M3_TARGET" = "hw" ]; then
    build_params_hw $script
else
    echo "Unknown target '$M3_TARGET'"
fi

if [ -f $build/$M3_FS.out ]; then
    $build/tools/m3fsck $build/$M3_FS.out && echo "FS image '$build/$M3_FS.out' is valid"
fi
