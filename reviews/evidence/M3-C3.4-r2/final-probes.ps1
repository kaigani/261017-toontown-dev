$ErrorActionPreference = 'Continue'
$evidence = 'reviews/evidence/M3-C3.4-r2'
function Run-ReviewCargo([string]$name, [string[]]$arguments) {
    $active = Get-Process cargo,rustc -ErrorAction SilentlyContinue
    tasklist /FI 'IMAGENAME eq cargo.exe' | Out-File "$evidence/$name-processes.log"
    tasklist /FI 'IMAGENAME eq rustc.exe' | Out-File "$evidence/$name-processes.log" -Append
    if ($active) { throw 'A cargo or rustc process is already running; refuse overlapping invocation' }
    Set-Content "$evidence/checks-current.txt" $name
    $started=Get-Date
    & cargo @arguments *> "$evidence/$name.log"
    $result=$LASTEXITCODE
    Set-Content "$evidence/$name.exit" $result
    "$name exit=$result elapsed=$((Get-Date)-$started)" | Add-Content "$evidence/checks-summary.txt"
}
Run-ReviewCargo 'direct-followup' @('test','--locked','-p','tt-executor','-j','4','--test','c34_r2_cancellation','direct_job_wait','--','--nocapture')
Run-ReviewCargo 'batch-followup' @('test','--locked','-p','tt-executor','-j','4','--test','c34_r2_measurements','--','--nocapture','--test-threads=1','--skip','one_hundred_wait','--skip','queued_cancelled_request','--skip','atomic_and_dry','--skip','discarded_queued')
Run-ReviewCargo 'fmt-final' @('fmt','-p','tt-executor','-p','tt-mcp','--check')
Run-ReviewCargo 'clippy-reviewer-final' @('clippy','--locked','-p','tt-executor','--test','c34_r2_cancellation','--test','c34_r2_measurements','-j','4','--','-D','warnings','-A','clippy::empty_line_after_doc_comments')
Set-Content "$evidence/final-probes.exit" 0
