Set-Location 'C:\Users\kaiga\work\review\m3-c33-r1'
$r=Join-Path $env:TEMP 'toontown-c33-r1'
while (!(Test-Path "$r\release.exit")) { Start-Sleep 5 }
$refs=@()
foreach($lib in @('tt_executor','tt_registry','tt_model','tt_persist','tt_render','tt_vector','tt_io','serde_json','sha2')) {
 $libPath=Get-ChildItem "target/release/deps/lib$lib-*.rlib" | Sort-Object LastWriteTime -Descending | Select-Object -First 1 -ExpandProperty FullName
 $refs+=@('--extern',"$lib=$libPath")
}
rustc --edition 2024 -O --test crates/executor/tests/c33_r1_pixels.rs @refs -L dependency=target/release/deps -o "$r\review-release-pixels.exe" *> "$r\review-release-pixels-compile.log"
$LASTEXITCODE | Set-Content "$r\review-release-pixels-compile.exit"
if($LASTEXITCODE -eq 0) {
 & "$r\review-release-pixels.exe" --nocapture *> "$r\review-release-pixels.log"
 $LASTEXITCODE | Set-Content "$r\review-release-pixels.exit"
}
