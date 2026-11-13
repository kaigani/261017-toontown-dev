Set-Location 'C:\Users\kaiga\work\review\m3-c33-r1'
$r=Join-Path $env:TEMP 'toontown-c33-r1'
while (!(Test-Path "$r\python-final.log")) { Start-Sleep 2 }
$hash=(Get-FileHash -Algorithm SHA256 tests/adversarial/c33-r1-audit.py).Hash.ToLowerInvariant()
@{sha256=$hash;startedAt=(Get-Date).ToUniversalTime().ToString('o')} | ConvertTo-Json | Set-Content "$r\python-final-source.json"
