# Part C B — host bypass evidence (charter 1)

Product ac2e9ff. Verdict: implemented for registry ownership of document mutations, with the sensor trust limitation below. This is a source audit, not proof that compromised local web content is harmless.

| Tauri command | State/effect | Registered executor path |
|---|---|---|
| ping | latency engine probe | no document mutation |
| app_info | engine info read | no mutation |
| run_command | arbitrary registered user command | Host::run -> invoke/invoke_served_until, Origin::User |
| cancel_request | request AtomicBool only | host cancellation API; not job.cancel |
| report_shell | modal/shared flags, text focus, pointer/description, screen geometry/clamping, view sizes, tree, rendered seq, message event | host-only sensor API, not registry; no document/history writes |
| dispatch_key | resolves key + completion of toggle parameters | resolve_key; returns reference; frontend subsequently run_command |
| pick_path | opens native picker, returns selected path | no file-content read/write, no document mutation; default_path only chooses picker directory |
| startup | consumes pending-open vector once | no file read/write, frontend subsequently project.open |
| restart_engine | replaces executor, releases old recovery locks, restarts HTTP, resets watchdog | host-only lifecycle; fixed user dirs from launch; no caller-supplied path |
| bench_plan | read launch diagnostic plan | diagnostic feature only |
| bench_report | writes report to launch-configured output and exits | diagnostic feature only; not registered document mutation |
| spike_mode | reads launch mode | diagnostic feature only |
| spike_environment | reads native environment | diagnostic feature only |
| spike_tablet_native_start | starts native packet stream | diagnostic feature only |
| spike_tablet_native_stop | stops native packet stream | diagnostic feature only |
| spike_pen_simulate | sends configured synthetic input | diagnostic feature only |
| spike_probe_native_window | native window probe | diagnostic feature only |
| spike_raw_input_sink | toggles diagnostic sink | diagnostic feature only |
| spike_save_log | writes caller JSON to launch-configured log path, may exit | diagnostic feature only; IPC cannot select output path |
| spike_canvas_scene | generates/caches diagnostic scene bytes | diagnostic feature only; no project state |
| spike_canvas_pixels | generates diagnostic pixel bytes | diagnostic feature only; no project state |

All command macros are in ipc.rs (9), diagnostics.rs (2), spikes/tablet/mod.rs (8), spikes/canvas/mod.rs (2). lib.rs command_registry has separate default and m0-diagnostics lists; no diagnostics commands in default registration.

Concrete sensor trust result: content with access to this Tauri IPC can send report_shell with a fabricated dialog or gesture. There is no user-presence check, source authentication beyond the webview boundary, timestamp or lease expiry. A silent reporter retains that state indefinitely; each agent action times out independently. This is a trust limitation, not agent control through a registered command. Such content can already invoke run_command as Origin::User; sensor forgery does not create a new privilege compared with that existing trusted shell capability. No arbitrary remote-content IPC access is inferred from mere existence of the command. Product CSP/capability audit is separately assigned and excluded here.

complete_reference is not a Tauri command: menu.rs:444 calls it to resolve missing toggle params. It reads kernel state and returns params, with no file path operation or document write (host.rs complete). startup cannot read the contents of returned pending paths. pick_path neither opens nor saves the chosen document. restart_engine does have recovery/lifecycle effects, but accepts no path and agents cannot call it through MCP.

MCP reachability correction: resolve_key **is** reached by the MCP key tool (server.rs:1092), as required by shared key dispatch; the ADR's blanket claim that none of the kernel APIs is reachable by agents is too broad. It resolves to a command then runs with session authorization. report_shell, complete_reference and set_wait_for_user have no MCP call sites; run_command looks up registered command names, so spelling these API names does not invoke them. No handler registers those names.

The accompanying invoke inventory records every literal invoke( occurrence, including tests, documentation examples and mocks. Production transport is tauriTransport.ts; engine.ts wraps run_command, cancel_request, dispatch_key, report_shell and pick_path. hostApi.ts generic call wraps bootstrap/startup/restart. spikeApi.ts contains diagnostics-only client wrappers. Other occurrences are test/mocks or boundary-rejection fixtures, not independent product mutation paths.
