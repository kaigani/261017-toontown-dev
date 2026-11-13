$ErrorActionPreference = 'Continue'
Set-Location 'C:\Users\kaiga\work\review\m3-c34-r5'
$dest='reviews/evidence/M3-C3.4-r5'
function Run($name, $arguments) {
  & cargo @arguments *> "$dest/$name.log"
  "$name $LASTEXITCODE" | Add-Content "$dest/exits.txt"
}
Run 'shape' @('test','--locked','-p','tt-executor','-j','4','--test','c34_r4_cancellation_shape','--','--nocapture')
Run 'late-debug' @('test','--locked','-p','tt-executor','-j','4','--test','c34_r4_late_probe','--','--nocapture')
Run 'late-release' @('test','--release','--locked','-p','tt-executor','-j','4','--test','c34_r4_late_probe','--','--nocapture')
1..3 | ForEach-Object { Run "closure-$_" @('test','--locked','-p','tt-executor','-j','4','--test','c34_r4_closure','--','--nocapture','--test-threads=1') }
1..10 | ForEach-Object { Run "cumulative-$_" @('test','--locked','-p','tt-executor','-j','4','--test','c34_round4','answers_just_inside_the_bound_hold_the_thread_at_most_the_budget_plus_one_bound','--','--exact','--nocapture') }
Run 'executor' @('test','--locked','-p','tt-executor','-j','4','--','--skip','killed_sequence_leaves_no_staged_files','--skip','two_hundred_answers_after_the_bound_must_latch_after_one_question')
Run 'mcp' @('test','--locked','-p','tt-mcp','-j','4')
Run 'stress' @('test','--release','--locked','-p','tt-mcp','-j','4','--test','c34_r3_stress','--','--nocapture')
Run 'headless' @('build','--release','--locked','-p','tt-headless','-j','4')
& .venv/Scripts/python.exe scripts/e1-bouncing-ball.py target/release/toontown-headless.exe "$dest/E1" *> "$dest/e1.log"
"e1 $LASTEXITCODE" | Add-Content "$dest/exits.txt"
'complete' | Set-Content "$dest/sequence.done"
