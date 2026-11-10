Set-Location 'C:\Users\kaiga\work\review\m3-c33-r1'
cargo test -p tt-render --locked -j 4 *> 'C:\Users\kaiga\AppData\Local\Temp\toontown-c33-r1\render.log'
$LASTEXITCODE | Set-Content 'C:\Users\kaiga\AppData\Local\Temp\toontown-c33-r1\render.exit'
