Set-Location 'C:/Users/kaiga/work/review/m3-c33-r2'
$r='C:/Users/kaiga/work/review/m3-c33-r2/reviews/evidence/M3-C3.3-r2'
while (!(Test-Path "$r/pipeline.exit")) { Start-Sleep 5 }
cargo test -p tt-executor --locked -j 4 --no-fail-fast --test c33_r2_depth --test c33_r2_recovery --test c33_r2_stress --test c33_r2_visibility --test c33_r2_wait *> "$r/final-probes.log"
$LASTEXITCODE | Set-Content "$r/final-probes.exit"
cargo clippy -p tt-executor --locked -j 4 --test c33_r2_depth --test c33_r2_recovery --test c33_r2_stress --test c33_r2_visibility --test c33_r2_wait -- -D warnings *> "$r/final-clippy.log"
$LASTEXITCODE | Set-Content "$r/final-clippy.exit"
rustfmt --check --edition 2024 crates/executor/tests/c33_r2_depth.rs crates/executor/tests/c33_r2_recovery.rs crates/executor/tests/c33_r2_stress.rs crates/executor/tests/c33_r2_visibility.rs crates/executor/tests/c33_r2_wait.rs *> "$r/final-fmt.log"
$LASTEXITCODE | Set-Content "$r/final-fmt.exit"
git diff --exit-code *> "$r/tracked-diff.log"
$LASTEXITCODE | Set-Content "$r/tracked-diff.exit"
'DONE' | Set-Content "$r/final-checks.exit"
