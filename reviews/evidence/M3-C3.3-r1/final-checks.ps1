Set-Location 'C:\Users\kaiga\work\review\m3-c33-r1'
$r=Join-Path $env:TEMP 'toontown-c33-r1'
while (!(Test-Path "$r\pipeline.exit")) { Start-Sleep 5 }
cargo fmt --all --check *> "$r\fmt.log"
$LASTEXITCODE | Set-Content "$r\fmt.exit"
cargo clippy -p tt-executor --test c33_r1_bitmap --test c33_r1_contract --test c33_r1_crash --test c33_r1_limits --test c33_r1_pixels --locked -j 4 -- -D warnings *> "$r\review-clippy-executor.log"
$rcExecutor=$LASTEXITCODE
$rcExecutor | Set-Content "$r\review-clippy-executor.exit"
cargo clippy -p tt-render --test c33_r1_cancel --locked -j 4 -- -D warnings *> "$r\review-clippy-render.log"
$rcRender=$LASTEXITCODE
$rcRender | Set-Content "$r\review-clippy-render.exit"
cargo clippy -p tt-mcp --test c33_r1_tools --locked -j 4 -- -D warnings *> "$r\review-clippy-mcp.log"
$rcMcp=$LASTEXITCODE
$rcMcp | Set-Content "$r\review-clippy-mcp.exit"
[math]::Max($rcExecutor,[math]::Max($rcRender,$rcMcp)) | Set-Content "$r\review-clippy.exit"
python scripts/run-reviewer-python-suites.py *> "$r\python-final.log"
$LASTEXITCODE | Set-Content "$r\python-final.exit"
python scripts/check-product-allows.py *> "$r\allows-final.log"
$LASTEXITCODE | Set-Content "$r\allows-final.exit"
python scripts/gen-command-declarations.py *> "$r\declarations-final.log"
$LASTEXITCODE | Set-Content "$r\declarations-final.exit"
git diff --exit-code *> "$r\post-test-tracked-diff.log"
$LASTEXITCODE | Set-Content "$r\post-test-tracked-diff.exit"
git status --short *> "$r\precommit-status.txt"
'DONE' | Set-Content "$r\final-checks.exit"
