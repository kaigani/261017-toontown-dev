$env:TT_COMMIT_REVIEW_RECEIPTS = $null
cargo test -p tt-mcp --locked -j 4 --test c33_r3_jobs --no-fail-fast *> reviews/evidence/M3-C3.3-r3/mcp.log
$LASTEXITCODE | Set-Content reviews/evidence/M3-C3.3-r3/mcp.exit
cargo clippy --workspace --all-targets --locked -- -D warnings -A clippy::empty_line_after_doc_comments *> reviews/evidence/M3-C3.3-r3/all-targets-clippy.log
$LASTEXITCODE | Set-Content reviews/evidence/M3-C3.3-r3/all-targets-clippy.exit
cargo clippy -p tt-executor --locked --test c33_r3_recovery --test c33_r3_transaction --test c33_r3_live --test c33_r2_wait --test c33_r2_recovery -- -D warnings *> reviews/evidence/M3-C3.3-r3/new-roots-clippy.log
$LASTEXITCODE | Set-Content reviews/evidence/M3-C3.3-r3/new-roots-clippy.exit
cargo clippy -p tt-mcp --locked --test c33_r3_jobs -- -D warnings *> reviews/evidence/M3-C3.3-r3/mcp-clippy.log
$LASTEXITCODE | Set-Content reviews/evidence/M3-C3.3-r3/mcp-clippy.exit
cargo fmt -p tt-executor --check *> reviews/evidence/M3-C3.3-r3/fmt.log
$LASTEXITCODE | Set-Content reviews/evidence/M3-C3.3-r3/fmt.exit
cargo fmt -p tt-mcp --check *>> reviews/evidence/M3-C3.3-r3/fmt.log
$LASTEXITCODE | Set-Content reviews/evidence/M3-C3.3-r3/mcp-fmt.exit
pnpm --dir desktop exec vitest run --config vite.config.ts src/test/c03-r1-tooling.test.ts *> reviews/evidence/M3-C3.3-r3/tooling-guard.log
$LASTEXITCODE | Set-Content reviews/evidence/M3-C3.3-r3/tooling-guard.exit
