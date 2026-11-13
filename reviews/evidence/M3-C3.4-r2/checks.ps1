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
Run-ReviewCargo 'reviewer' @('test','--locked','-p','tt-executor','-j','4','--test','c34_r2_cancellation','--test','c34_r2_measurements','--no-fail-fast','--','--nocapture','--test-threads=1')
Run-ReviewCargo 'wire' @('test','--locked','-p','tt-mcp','-j','4','--features','tt-persist/test-hooks','--test','c34_r2_protocol','--','--nocapture')
Run-ReviewCargo 'fmt' @('fmt','-p','tt-executor','-p','tt-mcp','--check')
Run-ReviewCargo 'clippy-product' @('clippy','--locked','-p','tt-executor','-p','tt-mcp','--lib','--bins','--examples','-j','4','--','-D','warnings')
Run-ReviewCargo 'clippy-targets' @('clippy','--locked','-p','tt-executor','-p','tt-mcp','--all-targets','--features','tt-persist/test-hooks','-j','4','--','-D','warnings','-A','clippy::empty_line_after_doc_comments')
$env:GITHUB_SHA='967229ad0bd172691771fe12252003439b4767fe'
Run-ReviewCargo 'release' @('build','--release','--locked','-p','tt-headless','-j','4')
Set-Content "$evidence/checks.exit" 0

