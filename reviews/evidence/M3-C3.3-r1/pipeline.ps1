Set-Location 'C:\Users\kaiga\work\review\m3-c33-r1'
$r=Join-Path $env:TEMP 'toontown-c33-r1'
while (!(Test-Path "$r\render.exit")) { Start-Sleep 5 }
cargo test -p tt-executor --locked -j 4 *> "$r\executor.log"
$LASTEXITCODE | Set-Content "$r\executor.exit"
cargo test -p tt-mcp --locked -j 4 *> "$r\mcp.log"
$LASTEXITCODE | Set-Content "$r\mcp.exit"
cargo clippy -p tt-executor --test c33_r1_contract --locked -j 4 -- -D warnings *> "$r\clippy.log"
$LASTEXITCODE | Set-Content "$r\clippy.exit"
cargo build --release --locked -p tt-headless -j 4 *> "$r\release.log"
$LASTEXITCODE | Set-Content "$r\release.exit"
python scripts/c33-render-gate.py target/release/toontown-headless.exe "$r\gate" *> "$r\gate.log"
$LASTEXITCODE | Set-Content "$r\gate.exit"
cargo deny check *> "$r\deny.log"
$LASTEXITCODE | Set-Content "$r\deny.exit"
Get-PSDrive C | Format-List Free | Out-File "$r\disk.txt"
if ((Get-PSDrive C).Free -gt 120GB) {
 cargo test --workspace --locked -j 4 --no-fail-fast *> "$r\workspace.log"
 $LASTEXITCODE | Set-Content "$r\workspace.exit"
}
'DONE' | Set-Content "$r\pipeline.exit"
