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

if [ "$M3_KERNEL" = "rustkernel" ]; then
    KPREFIX=rust
fi

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
    xmllint --xpath /config/app $2/boot-all.xml > $2/boot.xml || true
}

build_params_host() {
    generate_config $1 run || exit 1

    kargs=$(perl -ne '/<kernel\s.*args="(.*?)"/ && print $1' < run/boot-all.xml)
    mods=$(perl -ne 'printf(" '$bindir'/%s", $1) if /app\s.*args="([^\/"\s]+).*"/' < run/boot-all.xml)
    echo "$bindir/$KPREFIX$kargs run/boot.xml$mods"
}

build_params_gem5() {
    M3_GEM5_OUT=${M3_GEM5_OUT:-run}

    generate_config $1 $M3_GEM5_OUT || exit 1

    kargs=$(perl -ne '/<kernel\s.*args="(.*?)"/ && print $1' < $M3_GEM5_OUT/boot-all.xml)
    mods=$(perl -ne 'printf(",'$bindir'/%s", $1) if /app\s.*args="([^\/"\s]+).*"/' < $M3_GEM5_OUT/boot-all.xml)
    mods="$M3_GEM5_OUT/boot.xml$mods"

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

    cmd="$cmd$bindir/$KPREFIX$kargs,"
    c=0
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

    echo -n "--outdir=$M3_GEM5_OUT --debug-file=gem5.log --debug-flags=$M3_GEM5_DBG" >> $params
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

if [ "$M3_TARGET" = "host" ]; then
    params=$(build_params_host $script) || exit 1

    if [[ $params == *disk* ]] && [ "$M3_HDD" = "" ]; then
        ./src/tools/disk.py create $M3_HDD_PATH $build/$M3_FS
    fi

    if [ "$M3_VALGRIND" != "" ]; then
        valgrind $M3_VALGRIND $params
    else
        setarch $(uname -m) -R $params
    fi
elif [ "$M3_TARGET" = "gem5" ]; then
    build_params_gem5 $script
else
    echo "Unknown target '$M3_TARGET'"
fi

if [ -f $build/$M3_FS.out ]; then
    $build/src/tools/m3fsck/m3fsck $build/$M3_FS.out && echo "FS image '$build/$M3_FS.out' is valid"
fi
