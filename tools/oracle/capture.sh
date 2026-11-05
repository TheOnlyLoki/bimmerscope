#!/bin/sh
# Captures EdiabasLib reference output for golden tests.
# usage: capture.sh <ecu-dir> <sim-dir> <jobs-file> <out-dir>
# jobs-file: one "<sgbd> <job> [args]" per line, '#' comments allowed.
# Output: <out-dir>/<sgbd>__<job>.txt (gitignored: derived from BMW files).
set -eu
ecu=$1; sim=$2; jobs=$3; out=$4
here=$(dirname "$0")
mkdir -p "$out"
grep -vE '^\s*(#|$)' "$jobs" | while read -r sgbd job args; do
  f="$out/${sgbd}__${job}.txt"
  if [ -n "${args:-}" ]; then spec="$job#$args"; else spec="$job"; fi
  ext=prg; [ -e "$ecu/$sgbd.grp" ] && ext=grp
  "$here/run.sh" "$ecu" "$sim" -s "/ecu/$sgbd.$ext" -j "$spec" > "$f" 2>&1 < /dev/null || true
  echo "$sgbd $job: $(wc -l < "$f") lines"
done
