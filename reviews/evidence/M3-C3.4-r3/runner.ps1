$ErrorActionPreference='Continue'
Set-Location 'C:\Users\kaiga\work\review\m3-c34-r3'
$e='reviews/evidence/M3-C3.4-r3'
function Run-Cargo($name, $argv) {
 if (Get-Process cargo,rustc -ErrorAction SilentlyContinue) { throw 'Unexpected concurrent cargo/rustc' }
 & cargo @argv *> "$e/$name.log"
 $LASTEXITCODE | Set-Content "$e/$name.exit"
}
Run-Cargo 'fmt-write' @('fmt','-p','tt-executor','-p','tt-mcp')
Run-Cargo 'probes' @('test','--locked','-p','tt-executor','-j','4','--test','c34_r3_probes','--','--nocapture','--test-threads=1')
Run-Cargo 'mcp' @('test','--locked','-p','tt-mcp','-j','4','--features','tt-persist/test-hooks','--','--nocapture')
Run-Cargo 'executor' @('test','--locked','-p','tt-executor','-j','4','--no-fail-fast')
Run-Cargo 'vector' @('test','--locked','-p','tt-vector','-j','4','--features','test-hooks','--no-fail-fast')
Run-Cargo 'fmt' @('fmt','-p','tt-executor','-p','tt-mcp','-p','tt-vector','-p','tt-persist','--check')
Run-Cargo 'clippy-lib' @('clippy','--locked','-p','tt-executor','-p','tt-mcp','-p','tt-vector','-p','tt-persist','--lib','--bins','--examples','-j','4','--','-D','warnings')
Run-Cargo 'clippy-all' @('clippy','--locked','-p','tt-executor','-p','tt-mcp','-p','tt-vector','-p','tt-persist','--features','tt-persist/test-hooks','--all-targets','-j','4','--','-D','warnings','-A','clippy::empty_line_after_doc_comments')
Run-Cargo 'release' @('build','--release','--locked','-p','tt-headless','-j','4')
$env:PYTHONIOENCODING='utf-8'
$env:GITHUB_SHA='0146e1a26962a828c959c1054b612a2b76a50b0b'
& .venv/Scripts/python.exe scripts/e1-bouncing-ball.py target/release/toontown-headless.exe "$e/E1-final" *> "$e/e1.log"
$LASTEXITCODE | Set-Content "$e/e1.exit"
'done' | Set-Content "$e/runner.done"
