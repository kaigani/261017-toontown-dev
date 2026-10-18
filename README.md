# Toontown (working name)

Desktop 2-D animation application for Windows and macOS with full MCP agent control.

- `spec/` — the requirements specification. **Read-only in this repository**; only the orchestrator refreshes it from
  the authoritative copy.
- `plan/IMPLEMENTATION_PLAN.md` — stack, architecture, milestones M0–M11, build and review protocol.
- `reviews/` — adversarial review reports and builder responses per checkpoint.
- `docs/decisions/` — ADRs, one per design decision taken from the spec's decision lists.
- `docs/manual-tests/` — step-by-step scripts for checks that need a human (macOS tablet tests etc.).
- `tests/adversarial/` — failing tests committed by the reviewer; kept permanently.

Stack: Rust engine crates + Tauri 2 shell + TypeScript/React front end. See the plan for details.
