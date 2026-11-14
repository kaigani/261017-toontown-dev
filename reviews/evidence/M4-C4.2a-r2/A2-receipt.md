Candidate a2b5101, Windows, debug, isolated test threads.

- Extreme mapping: pass.
- Ordered snapshot/edit: pass.
- Six captures: six peak unfinished jobs, all PNGs equal.
- 40ms budget: job.waitTimeout, 43.6586ms elapsed.
- Ten request cancellations: ten command.timeout / cancelled responses; ten jobs succeeded (expected cancelled).
- OS thread counts before/after: 7 / 7; all jobs awaited.
- Original latency and PNG/renderer comparison: pass.
