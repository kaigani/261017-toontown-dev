Set-Location 'C:\Users\kaiga\work\review\m3-c33-r1'
$r=Join-Path $env:TEMP 'toontown-c33-r1'
while (!(Test-Path "$r\release.exit")) { Start-Sleep 5 }
$refs=@()
foreach($lib in @('tt_executor','tt_registry','tt_model','tt_persist','tt_render','tt_vector','serde_json','sha2')) {
 $libPath=Get-ChildItem "target/release/deps/lib$lib-*.rlib" | Sort-Object LastWriteTime -Descending | Select-Object -First 1 -ExpandProperty FullName
 $refs+=@('--extern',"$lib=$libPath")
}
$env:CARGO_MANIFEST_DIR='C:\Users\kaiga\work\review\m3-c33-r1\crates\executor'
rustc --edition 2024 -O --test crates/executor/tests/c33_goldens.rs @refs -L dependency=target/release/deps -o "$r\release-goldens.exe" *> "$r\release-goldens-compile.log"
if($LASTEXITCODE -eq 0) {
 & "$r\release-goldens.exe" --nocapture *> "$r\release-goldens.log"
 $LASTEXITCODE | Set-Content "$r\release-goldens.exit"
}
