Candidate a2b5101, Windows debug.

- Executor new suite: 6 pass.
- MCP new suite: 1 pass.
- Five writer families: 40ms waits reject ui.busy; state queries unchanged.
- Agent queries and user writers: inside 1s with waitForUserMs=2s.
- Pointer under user gesture: ui.busy; clearing gesture during wait permits agent tool change.
- Scope combinations: missing input, missing edit, full scopes; expected outcomes pass.
- 10,000-event no-input refusal: within 2s transport margin.
- C4.1 executor priority: 2 pass; part_c1: 6 pass.
- C4.1 MCP r2: 14 pass; r3: 8 pass.
- Formatting and requested all-target lint checks passed.
