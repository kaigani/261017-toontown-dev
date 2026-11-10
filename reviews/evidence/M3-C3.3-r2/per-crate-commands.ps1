Set-Location 'C:/Users/kaiga/work/review/m3-c33-r2'
$r='C:/Users/kaiga/work/review/m3-c33-r2/reviews/evidence/M3-C3.3-r2'
cargo test -p tt-render --locked -j 4 *> "$r/render.log"
$LASTEXITCODE | Set-Content "$r/render.exit"
cargo test -p tt-executor --locked -j 4 --test c33_round1 --test c33_r2_recovery --test c33_r1_contract --test c33_r1_limits *> "$r/executor.log"
$LASTEXITCODE | Set-Content "$r/executor.exit"
cargo test -p tt-mcp --locked -j 4 *> "$r/mcp.log"
$LASTEXITCODE | Set-Content "$r/mcp.exit"
'DONE' | Set-Content "$r/percrate.exit"
