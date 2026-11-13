Set-Location 'C:\Users\kaiga\work\review\m3-c33-r1'
$r=Join-Path $env:TEMP 'toontown-c33-r1'
while (!(Test-Path "$r\workspace.log")) { Start-Sleep 2 }
$hashes=@{}
Get-ChildItem crates/*/tests/c33_r1_*.rs,tests/adversarial/c33-r1-*.py | ForEach-Object { $hashes[$_.FullName]=(Get-FileHash -Algorithm SHA256 -LiteralPath $_.FullName).Hash.ToLowerInvariant() }
@{head=(git rev-parse HEAD);argv='cargo test --workspace --locked -j 4 --no-fail-fast';startedAt=(Get-Date).ToUniversalTime().ToString('o');sourceHashes=$hashes;freeBytes=(Get-PSDrive C).Free;receiptEnvironmentSet=(Test-Path Env:TT_COMMIT_REVIEW_RECEIPTS)} | ConvertTo-Json -Depth 5 | Set-Content "$r\workspace-metadata.json"
