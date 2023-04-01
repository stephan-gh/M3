#!/bin/sh

MAKE_ARGS="-j"$(nproc)

usage() {
    echo "Usage: $1 (x86_64|arm|riscv) ..." >&2
    exit
}

if [ $# -lt 1 ]; then
    usage "$0"
fi

ARCH="$1"
shift
if [ "$ARCH" != "x86_64" ] && [ "$ARCH" != "arm" ] && [ "$ARCH" != "riscv" ]; then
    usage "$0"
fi

ROOT=$(dirname "$(readlink -f "$0")")
DIST="$(readlink -f "$ROOT/..")/build/cross-$ARCH"

if [ -f "$DIST/.config" ] && [ "$(cmp "$DIST/.config-origin" "config-$ARCH" 2>/dev/null)" != "" ]; then
    printf "\e[1mWARNING: %s/.config-origin and config-%s differ\n\e[0m" "$DIST" "$ARCH"
    printf "This probably indicates that config-%s was updated and you should rebuild.\n" "$ARCH"
    printf "Do you want to rebuild (r) or continue (c) with the potentially outdated %s/.config? " "$DIST"
    read -r choice
    case $choice in
        r) rm -rf "$DIST" ;;
        c) ;;
        *) exit ;;
    esac
fi

if [ ! -f "$DIST/.config" ]; then
    ( cd buildroot && make O="$DIST" "$MAKE_ARGS" defconfig "BR2_DEFCONFIG=../config-$ARCH" )
    cp "config-$ARCH" "$DIST/.config-origin"
fi

( cd buildroot && make O="$DIST" "$MAKE_ARGS" "$@" )
