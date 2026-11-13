$ErrorActionPreference = 'Continue'
Set-Location 'C:\Users\kaiga\work\review\m3-c34-r5'
$d='reviews/evidence/M3-C3.4-r5'
function Run($name,$arguments) { & cargo @arguments *> "$d/$name.log"; "$name $LASTEXITCODE" | Add-Content "$d/exits.txt" }
Run 'mcp-ci-features' @('test','--locked','-p','tt-mcp','--features','tt-persist/test-hooks','-j','4')
Run 'r5-probes' @('test','--locked','-p','tt-executor','-j','4','--test','c34_r5_deadlines','--test','c34_r5_shapes','--no-fail-fast','--','--nocapture','--test-threads=1')
Run 'queued-job' @('test','--locked','-p','tt-executor','-j','4','--test','c34_round4','a_queued_batch_wait_is_bounded_from_its_callers_enqueue_not_from_its_start','--','--exact','--nocapture')
Run 'executor-tail' @('test','--locked','-p','tt-executor','-j','4','--test','c34_round2','--test','c34_round3','--test','c34_round4','--test','c34_tokens','--test','compile_fail')
Run 'executor-doc' @('test','--doc','--locked','-p','tt-executor','-j','4')
Run 'fmt-executor' @('fmt','-p','tt-executor','--check')
Run 'fmt-vector' @('fmt','-p','tt-vector','--check')
Run 'clippy' @('clippy','--workspace','--lib','--bins','--examples','--locked','-j','4','--','-D','warnings')
Run 'clippy-all' @('clippy','--workspace','--all-targets','--locked','-j','4','--','-D','warnings','-A','clippy::empty_line_after_doc_comments')
& .venv/Scripts/python.exe scripts/check-release-features.py *> "$d/features.log"
"features $LASTEXITCODE" | Add-Content "$d/exits.txt"
