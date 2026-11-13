$env:TT_COMMIT_REVIEW_RECEIPTS = $null
$ev = 'reviews/evidence/M3-C3.3-r4'
cargo test -p tt-executor --locked -j 4 --test c33_r4_closure -- --test-threads=1 *> "$ev/new-executor.log"
$LASTEXITCODE | Set-Content "$ev/new-executor.exit"
cargo test -p tt-mcp --locked -j 4 --test c33_r4_closure *> "$ev/new-mcp.log"
$LASTEXITCODE | Set-Content "$ev/new-mcp.exit"
