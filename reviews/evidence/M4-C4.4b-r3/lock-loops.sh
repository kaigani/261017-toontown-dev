#!/usr/bin/env bash
# Ruling D-84 / D-85 receipts: the randomised lock test and the M1 test, each run TOTAL times, by WORKERS loops at once
# (each its own slice of the run numbers; each run uses its own temporary project folder). Mode `busy` adds one CPU
# burner per logical core at normal priority; `idle` adds nothing. Binaries are copies, so builds cannot collide with
# them. Every run's number, seed (the randomised test) and verdict are logged; a failure keeps its whole output.
# Usage: par.sh <idle|busy> <total> <workers>
MODE=$1
TOTAL=$2
WORKERS=$3
D=/c/Users/kaiga/AppData/Local/Temp/claude/317c361e32dd50ad/14a3c024-788e-4ade-8835-d20b9b95f8ff/scratchpad/lockloop
BIN=$D/bin
OUT=$D/$MODE
rm -f "$OUT"/*.runs "$OUT"/fail-* "$OUT"/*.summary.json 2>/dev/null
mkdir -p "$OUT"
burners=()
# The burners end with the script, however it ends.
trap 'for p in "${burners[@]}"; do kill "$p" 2>/dev/null; done' EXIT
if [ "$MODE" = busy ]; then
  for _ in $(seq "$(nproc)"); do
    python -c "while True: pass" &
    burners+=($!)
  done
  echo "burners ${#burners[@]}"
fi
seed_of() { local s=$((0x9E3779B97F4A7C15 ^ ($1 * 2654435761))); echo "${s#-}"; }
worker() {
  local name=$1 exe=$2 filter=$3 seeded=$4 w=$5
  for ((i = w + 1; i <= TOTAL; i += WORKERS)); do
    local seed
    seed=$(seed_of "$i")
    local log="$OUT/last-$name-$w.txt"
    if [ "$seeded" = 1 ]; then
      TT_LOCK_SEED=$seed "$exe" --exact "$filter" --test-threads=1 > "$log" 2>&1
    else
      "$exe" --exact "$filter" --test-threads=1 > "$log" 2>&1
    fi
    if [ $? -eq 0 ]; then
      echo "$i $seed pass" >> "$OUT/$name-$w.runs"
    else
      echo "$i $seed FAIL" >> "$OUT/$name-$w.runs"
      cp "$log" "$OUT/fail-$name-$i.txt"
    fi
  done
}
run_all() {
  local name=$1 exe=$2 filter=$3 seeded=$4
  local start=$(date +%s)
  local pids=()
  for ((w = 0; w < WORKERS; w++)); do
    worker "$name" "$exe" "$filter" "$seeded" "$w" &
    pids+=($!)
  done
  wait "${pids[@]}"
  local secs=$(( $(date +%s) - start ))
  local runs pass fail failed
  runs=$(cat "$OUT"/$name-*.runs | wc -l)
  pass=$(cat "$OUT"/$name-*.runs | grep -c " pass$")
  fail=$(cat "$OUT"/$name-*.runs | grep -c " FAIL$")
  failed=$(cat "$OUT"/$name-*.runs | grep " FAIL$" | awk '{print $2}' | tr '\n' ' ')
  echo "{\"test\": \"$filter\", \"mode\": \"$MODE\", \"workers\": $WORKERS, \"burners\": ${#burners[@]}, \"runs\": $runs, \"passed\": $pass, \"failed\": $fail, \"failedSeeds\": \"${failed% }\", \"seconds\": $secs}" | tee "$OUT/$name.summary.json"
}
run_all randomised "$BIN/c44b_lock_takeover.exe" many_contenders_with_jitter_leave_exactly_one_owner_whose_handle_stays_valid 1
run_all m1 "$BIN/c13_r1_persistence.exe" concurrent_stale_lock_takeover_has_only_one_owner 0
for p in "${burners[@]}"; do kill "$p" 2>/dev/null; done
echo "LOOP DONE $MODE"
