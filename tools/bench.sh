#!/usr/bin/env bash

if [ $# -ne 1 ] && [ $# -ne 2 ]; then
    echo "Usage: $0 <mhz> [<warmup>]" 1>&2
    echo "  Expects the gem5 log in stdin." 1>&2
    exit 1
fi

mhz=$1
warmup=0
if [ "$2" != "" ]; then
    warmup=$2
fi
starttsc="1ff1"
stoptsc="1ff2"

awk -v "warmup=$warmup" -v "mhz=$mhz" '
function handle(msg, tile, time) {
    id = substr(msg,7,4)
    idx = sprintf("%d.%s", tile, id)
    if(substr(msg,3,4) == "'$starttsc'") {
        start[idx] = time
    }
    else if(substr(msg,3,4) == "'$stoptsc'") {
        counter[idx] += 1
        if(counter[idx] > warmup)
            printf("Tile%d-TIME: %04s : %d cycles\n", tile, id, strtonum(time) - strtonum(start[idx]))
    }
}

function ticksToCycles(ticks) {
    return ticks * (mhz / 1000000)
}

/DMA-DEBUG-MESSAGE:/ {
    match($4, /^([[:digit:]]+)\.[[:digit:]]+\/[[:digit:]]+:$/, m)
    handle($7, m[1])
}

/DEBUG [[:xdigit:]]+/ {
    match($1, /^([[:digit:]]+):/, time)
    match($2, /(tile|cpu)([[:digit:]]+)/, tile)
    handle($4, tile[2], ticksToCycles(time[1]))
}
'
