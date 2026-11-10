Set-Location 'C:/Users/kaiga/work/review/m3-c33-r2'
$r='C:/Users/kaiga/work/review/m3-c33-r2/reviews/evidence/M3-C3.3-r2'
while (!(Test-Path "$r/percrate.exit")) { Start-Sleep 5 }
cargo test -p tt-executor --locked -j 4 --test c33_r2_depth *> "$r/depth.log"
$LASTEXITCODE | Set-Content "$r/depth.exit"
cargo clippy -p tt-executor --test c33_r2_depth --test c33_r2_recovery --locked -j 4 -- -D warnings *> "$r/clippy.log"
$LASTEXITCODE | Set-Content "$r/clippy.exit"
cargo deny check *> "$r/deny.log"
$LASTEXITCODE | Set-Content "$r/deny.exit"
Get-PSDrive C | Format-List Free | Out-File "$r/disk.txt"
if ((Get-PSDrive C).Free -gt 120GB) {
 cargo test --workspace --locked -j 4 --no-fail-fast *> "$r/workspace.log"
 $LASTEXITCODE | Set-Content "$r/workspace.exit"
}
'DONE' | Set-Content "$r/pipeline.exit"
