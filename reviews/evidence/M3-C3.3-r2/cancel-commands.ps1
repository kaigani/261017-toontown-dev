Set-Location 'C:/Users/kaiga/work/review/m3-c33-r2'
$r='C:/Users/kaiga/work/review/m3-c33-r2/reviews/evidence/M3-C3.3-r2'
while (!(Test-Path "$r/final-checks.exit")) { Start-Sleep 5 }
cargo test -p tt-render --locked -j 4 --test c33_r2_cancel *> "$r/cancel.log"
$LASTEXITCODE | Set-Content "$r/cancel.exit"
cargo clippy -p tt-render --locked -j 4 --test c33_r2_cancel -- -D warnings *> "$r/cancel-clippy.log"
$LASTEXITCODE | Set-Content "$r/cancel-clippy.exit"
rustfmt --check --edition 2024 crates/render/tests/c33_r2_cancel.rs *> "$r/cancel-fmt.log"
$LASTEXITCODE | Set-Content "$r/cancel-fmt.exit"
'DONE' | Set-Content "$r/all-checks.exit"
