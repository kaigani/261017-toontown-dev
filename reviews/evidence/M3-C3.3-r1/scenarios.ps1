Set-Location 'C:\Users\kaiga\work\review\m3-c33-r1'
$r=Join-Path $env:TEMP 'toontown-c33-r1'
while (!(Test-Path "$r\release.exit")) { Start-Sleep 5 }
$env:PYTHONIOENCODING='utf-8'
python scripts/e2-paint-by-id.py target/release/toontown-headless.exe "$r\e2" *> "$r\e2.log"
$LASTEXITCODE | Set-Content "$r\e2.exit"
python scripts/e4-cutout-numeric.py target/release/toontown-headless.exe "$r\e4" *> "$r\e4.log"
$LASTEXITCODE | Set-Content "$r\e4.exit"
python scripts/e4-cutout-numeric.py --variant target/release/toontown-headless.exe "$r\e4-variant" *> "$r\e4-variant.log"
$LASTEXITCODE | Set-Content "$r\e4-variant.exit"
python scripts/mcp-conformance.py target/release/toontown-headless.exe *> "$r\sdk.log"
$LASTEXITCODE | Set-Content "$r\sdk.exit"
'DONE' | Set-Content "$r\scenarios.exit"
