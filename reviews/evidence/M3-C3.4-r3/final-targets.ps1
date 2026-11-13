$ErrorActionPreference='Continue'
Set-Location 'C:\Users\kaiga\work\review\m3-c34-r3'
$e='reviews/evidence/M3-C3.4-r3'
function Verify-Idle {
 tasklist /FI 'IMAGENAME eq cargo.exe'
 tasklist /FI 'IMAGENAME eq rustc.exe'
 if (Get-Process cargo,rustc -ErrorAction SilentlyContinue) { throw 'Concurrent cargo/rustc' }
}
Verify-Idle *> "$e/final-targets-process-start.log"
& cargo test --locked -p tt-executor -j 4 --test c34_r3_contention --test c34_r3_edges --test c34_r3_probes --no-fail-fast -- --nocapture --test-threads=1 *> "$e/new-tests-final.log"
$LASTEXITCODE | Set-Content "$e/new-tests-final.exit"
Verify-Idle *> "$e/stress-process-start.log"
& cargo test --locked -p tt-mcp -j 4 --features tt-persist/test-hooks --test c34_r3_stress -- --nocapture *> "$e/stress-final.log"
$LASTEXITCODE | Set-Content "$e/stress-final.exit"
Verify-Idle *> "$e/feature-process-start.log"
$env:PYTHONIOENCODING='utf-8'
& .venv/Scripts/python.exe scripts/check-release-features.py *> "$e/release-features.log"
$LASTEXITCODE | Set-Content "$e/release-features.exit"
$env:GITHUB_SHA='0146e1a26962a828c959c1054b612a2b76a50b0b'
& .venv/Scripts/python.exe "$e/sdk-r3.py" *> "$e/sdk.log"
$LASTEXITCODE | Set-Content "$e/sdk.exit"
& .venv/Scripts/python.exe "$e/summarize.py" *> "$e/summary.log"
& .venv/Scripts/python.exe "$e/source-audit.py" *> "$e/source-audit.log"
Verify-Idle *> "$e/process-final.log"
'done' | Set-Content "$e/final-targets.done"
