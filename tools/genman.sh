#!/usr/bin/env bash

progs="basename cat cp cut date dd dirname du find head ln ls"
progs="$progs mkdir mktemp mv printenv printf pwd rm rmdir sleep stat sync"
progs="$progs tail tee test tr uniq wc"

mkdir -p src/fs/default/man

for p in $progs; do
    for no in 1 6 8; do
        f=src/apps/bsdutils/src/$p/$p.$no
        if [ -f "$f" ]; then
            MANWIDTH=100 man --ascii -E ascii "$f" > "src/fs/default/man/$(basename -s ".$no" "$f").1";
        fi
    done
done
