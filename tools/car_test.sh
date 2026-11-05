#!/bin/sh
# First car test: runs IDENT and FS_LESEN (read only) on a few modules and
# saves results, telegrams and traces to ./car-logs/<time>/.
# usage: tools/car_test.sh <ecu-dir> [port]
set -u
ecu=${1:?usage: tools/car_test.sh <ecu-dir> [port]}
port=${2:-/dev/ttyUSB0}
out=car-logs/$(date +%Y%m%d-%H%M%S)
mkdir -p "$out"
cargo build --release -q -p obd --example car_job || exit 1
bin=target/release/examples/car_job

run() {
    name=$1-$2
    echo "== $1 $2"
    BMWA_TELEGRAM_FILE="$out/$name.telegrams" BMWA_TRACE_FILE="$out/$name.trace" \
        timeout 60 "$bin" "$ecu" "$port" "$1" "$2" >"$out/$name.txt" 2>&1
    echo "   exit $?, $(grep -c . "$out/$name.txt") lines"
    grep -m1 -E "JOB_STATUS|IFH-|error" "$out/$name.txt" | sed 's/^/   /'
}

for sgbd in jbbf87 d_motor d_kombi d_dsc d_szl; do
    run "$sgbd" IDENT
done
for sgbd in jbbf87 d_motor; do
    run "$sgbd" FS_LESEN
done

tar czf "$out.tar.gz" "$out"
echo "Done. Send this file: $out.tar.gz"
