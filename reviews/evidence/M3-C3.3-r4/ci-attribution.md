# CI budget attribution

Durations from authenticated run metadata and downloaded logs. Test durations are harness wall time per binary, excluding nested summaries, which are retained separately in JSON; compile durations come from Cargo finish lines.

| Windows check step | 42915011320 s | 42938979758 s | Delta s |
|---|---:|---:|---:|
| Set up job | 3 | 2 | -1 |
| Run actions/checkout@v7 | 13 | 23 | +10 |
| Rust toolchain (pinned in rust-toolchain.toml) | 9 | 11 | +2 |
| Run Swatinem/rust-cache@v2 | 39 | 34 | -5 |
| Run pnpm/action-setup@v6 | 14 | 17 | +3 |
| Run actions/setup-node@v7 | 7 | 11 | +4 |
| Run pnpm install --frozen-lockfile | 3 | 2 | -1 |
| Fetch the locked dependencies | 25 | 35 | +10 |
| Front end — lint, type-check, test, format, licences | 138 | 182 | +44 |
| Reviewer Python suites (every tests/adversarial/*.py) | 18 | 22 | +4 |
| Front end — build, diagnostics variant then default (frontendDist is embedded by the Tauri host) | 5 | 7 | +2 |
| Formatting, per workspace package | 6 | 7 | +1 |
| Generated command declarations are current (C12-R1-12) | 1 | 2 | +1 |
| Run cargo clippy --workspace --lib --bins --examples --locked -- -D warnings | 79 | 101 | +22 |
| Run cargo test --workspace --locked | 1752 | 2069 | +317 |
| Desktop host with the M0 diagnostics feature (C0.4 gating) — clippy and tests | 172 | 228 | +56 |
| Run taiki-e/install-action@v2 | 3 | 4 | +1 |
| Run cargo deny --locked check | 3 | 4 | +1 |
| Test-only assets stay out of the product | 1 | 0 | -1 |
| C22-T2 gate analyser rejects malformed evidence | 2 | 3 | +1 |
| MCP protocol conformance with the official Python SDK client (C1.4; 11 §14.7), measurements | 142 | 186 | +44 |
| Run actions/upload-artifact@v7 | 2 | 2 | +0 |
| Post Run actions/setup-node@v7 | 0 | 1 | +1 |
| Post Run pnpm/action-setup@v6 | 0 | 0 | +0 |
| Post Run Swatinem/rust-cache@v2 | 0 | 0 | +0 |
| Post Run actions/checkout@v7 | 2 | 3 | +1 |
| Complete job | 1 | 0 | -1 |

| Job | Old Windows | New Windows | Old macOS | New macOS |
|---|---:|---:|---:|---:|
| check | 40m41s | 49m19s | 33m17s | 29m45s |
| c33 render release | 10m58s | 10m19s | 10m08s | 9m41s |
| review properties release | 38m17s | 31m09s | 27m26s | 25m13s |
| history oracles release | 20m25s | 20m31s | 14m05s | 14m51s |

## Windows per-binary test harness durations

Repeated filenames are disambiguated by occurrence in execution order; raw rows retain step labels. A missing occurrence is shown as absent, not zero.

| Binary occurrence | Old seconds | New seconds | Delta |
|---|---:|---:|---:|
| ../../tests/adversarial/c01.rs #1 | 5.26 | 6.96 | 1.7 |
| ../../tests/adversarial/c03-r2-encoder.rs #1 | 0.61 | 0.78 | 0.17 |
| ../../tests/adversarial/c03-r2-encoder.rs #2 | 0.59 | 0.8 | 0.21 |
| Doc-tests tt_anim #1 | 0.0 | 0.0 | 0.0 |
| Doc-tests tt_audio #1 | 0.0 | 0.0 | 0.0 |
| Doc-tests tt_colour #1 | 0.0 | 0.0 | 0.0 |
| Doc-tests tt_desktop #1 | 0.0 | 0.0 | 0.0 |
| Doc-tests tt_desktop #2 | absent | 0.0 | new/absent |
| Doc-tests tt_engine #1 | 0.0 | 0.0 | 0.0 |
| Doc-tests tt_executor #1 | 0.0 | 0.0 | 0.0 |
| Doc-tests tt_fx #1 | 0.0 | 0.0 | 0.0 |
| Doc-tests tt_io #1 | 0.0 | 0.0 | 0.0 |
| Doc-tests tt_lipsync #1 | 0.0 | 0.0 | 0.0 |
| Doc-tests tt_mcp #1 | 0.0 | 0.0 | 0.0 |
| Doc-tests tt_model #1 | 0.0 | 0.0 | 0.0 |
| Doc-tests tt_persist #1 | 0.0 | 0.0 | 0.0 |
| Doc-tests tt_registry #1 | 0.0 | 0.0 | 0.0 |
| Doc-tests tt_registry_macros #1 | 0.0 | 0.0 | 0.0 |
| Doc-tests tt_render #1 | 0.0 | 0.0 | 0.0 |
| Doc-tests tt_vector #1 | absent | 0.0 | new/absent |
| nested/doc #1 | 0.0 | absent | new/absent |
| nested/doc #2 | 0.0 | absent | new/absent |
| tests/audit.rs #1 | 1.52 | 1.59 | 0.07 |
| tests/c03_fixed_configurations.rs #1 | 1.5 | 1.71 | 0.21 |
| tests/c03_r1_stress.rs #1 | 1.85 | 2.18 | 0.33 |
| tests/c03_r2_quality.rs #1 | 1.29 | 1.45 | 0.16 |
| tests/c11_r1_adversarial.rs #1 | 1.53 | 1.76 | 0.23 |
| tests/c11_r1_wire.rs #1 | 0.01 | 0.01 | 0.0 |
| tests/c11_r2_adversarial.rs #1 | 2.02 | 2.52 | 0.5 |
| tests/c11_r2_wire.rs #1 | 0.0 | 0.0 | 0.0 |
| tests/c11_r3_adversarial.rs #1 | 0.11 | 0.17 | 0.06 |
| tests/c11_r3_random.rs #1 | 0.71 | 0.92 | 0.21 |
| tests/c11_r3_register.rs #1 | 0.19 | 0.27 | 0.08 |
| tests/c11_r3_wire.rs #1 | 0.0 | 0.01 | 0.01 |
| tests/c11_r4_exposure_bounds.rs #1 | 0.0 | 0.0 | 0.0 |
| tests/c11_r5_write_bounds.rs #1 | 0.0 | 0.0 | 0.0 |
| tests/c11_r6_staged_inverse.rs #1 | 0.11 | 0.16 | 0.05 |
| tests/c12_checkpoint_property.rs #1 | 1.19 | 1.36 | 0.17 |
| tests/c12_edit_ops.rs #1 | 0.38 | 0.52 | 0.14 |
| tests/c12_invalidation_property.rs #1 | 0.82 | 0.92 | 0.1 |
| tests/c12_kernel_state.rs #1 | 24.33 | 30.46 | 6.13 |
| tests/c12_kernel_support.rs #1 | 0.12 | 0.17 | 0.05 |
| tests/c12_never_block.rs #1 | 0.83 | 0.86 | 0.03 |
| tests/c12_policy.rs #1 | 0.25 | 0.37 | 0.12 |
| tests/c12_r1_contracts.rs #1 | 18.53 | 24.34 | 5.81 |
| tests/c12_r1_macro.rs #1 | 0.0 | 0.0 | 0.0 |
| tests/c12_r1_scale.rs #1 | 20.84 | 22.34 | 1.5 |
| tests/c12_r1_stress.rs #1 | 18.73 | 23.83 | 5.1 |
| tests/c12_r2_contracts.rs #1 | absent | 3.68 | new/absent |
| tests/c12_r2_registry.rs #1 | 0.12 | 0.2 | 0.08 |
| tests/c12_r2_scale.rs #1 | 24.17 | 26.88 | 2.71 |
| tests/c12_r3_contracts.rs #1 | 1.54 | 1.69 | 0.15 |
| tests/c12_r3_registry.rs #1 | 0.14 | 0.22 | 0.08 |
| tests/c12_r3_stress.rs #1 | 18.77 | 24.54 | 5.77 |
| tests/c12_r4_closure.rs #1 | 0.17 | 0.29 | 0.12 |
| tests/c12_registry.rs #1 | 0.15 | 0.26 | 0.11 |
| tests/c12_transactions.rs #1 | 0.64 | 0.89 | 0.25 |
| tests/c13_doc.rs #1 | 3.71 | 3.91 | 0.2 |
| tests/c13_projects.rs #1 | 6.31 | 6.62 | 0.31 |
| tests/c13_r1_format.rs #1 | 1.7 | 1.81 | 0.11 |
| tests/c13_r1_persistence.rs #1 | 2.71 | 3.25 | 0.54 |
| tests/c13_r2_format.rs #1 | 2.71 | 3.3 | 0.59 |
| tests/c13_r2_persistence.rs #1 | 4.36 | 5.88 | 1.52 |
| tests/c13_r3_format.rs #1 | 2.21 | 2.21 | 0.0 |
| tests/c13_r3_history_r1.rs #1 | 0.0 | 0.0 | 0.0 |
| tests/c13_r3_history_r3.rs #1 | 0.0 | 0.0 | 0.0 |
| tests/c13_r3_persistence.rs #1 | 5.02 | 4.87 | -0.15 |
| tests/c13_r4_closure.rs #1 | 0.25 | 0.38 | 0.13 |
| tests/c13_r4_lock.rs #1 | 1.08 | 1.09 | 0.01 |
| tests/c14_lock_races.rs #1 | 16.68 | 17.48 | 0.8 |
| tests/c14_measurements.rs #1 | 21.28 | 25.01 | 3.73 |
| tests/c14_obligations.rs #1 | 5.07 | 5.32 | 0.25 |
| tests/c14_r1.rs #1 | 3.37 | 3.95 | 0.58 |
| tests/c14_r1_autosave.rs #1 | 4.19 | 5.63 | 1.44 |
| tests/c14_r1_limits.rs #1 | 0.09 | 0.16 | 0.07 |
| tests/c14_r2.rs #1 | 14.89 | 16.04 | 1.15 |
| tests/c14_r2_contracts.rs #1 | 0.44 | 0.61 | 0.17 |
| tests/c14_r2_effects.rs #1 | 0.21 | 0.34 | 0.13 |
| tests/c14_r2_replay.rs #1 | 0.28 | 0.39 | 0.11 |
| tests/c14_r3.rs #1 | 3.56 | 4.52 | 0.96 |
| tests/c14_r3_groups.rs #1 | 0.27 | 0.37 | 0.1 |
| tests/c14_r3_history_r1.rs #1 | 0.0 | 0.0 | 0.0 |
| tests/c14_r3_history_r3.rs #1 | 0.0 | 0.0 | 0.0 |
| tests/c14_r3_replay.rs #1 | 0.39 | 0.53 | 0.14 |
| tests/c14_r4_closure.rs #1 | 0.27 | 0.45 | 0.18 |
| tests/c14_r4_expect_rev.rs #1 | 0.26 | 0.38 | 0.12 |
| tests/c14_r5_closure.rs #1 | 0.27 | 0.37 | 0.1 |
| tests/c14_tickets.rs #1 | 0.14 | 0.23 | 0.09 |
| tests/c21_determinism.rs #1 | 0.48 | 0.69 | 0.21 |
| tests/c21_draw.rs #1 | 0.28 | 0.46 | 0.18 |
| tests/c21_example_results.rs #1 | 11.01 | 14.46 | 3.45 |
| tests/c21_history.rs #1 | 0.0 | 0.0 | 0.0 |
| tests/c21_measurements.rs #1 | 0.0 | 0.0 | 0.0 |
| tests/c21_obligations.rs #1 | 2.71 | 3.01 | 0.3 |
| tests/c21_r1_autosave.rs #1 | 25.4 | 27.96 | 2.56 |
| tests/c21_r1_contracts.rs #1 | 0.2 | 0.33 | 0.13 |
| tests/c21_r1_geometry.rs #1 | 0.25 | 0.34 | 0.09 |
| tests/c21_r1_retention.rs #1 | 22.82 | 24.68 | 1.86 |
| tests/c21_r1_surfaces.rs #1 | 2.87 | 3.1 | 0.23 |
| tests/c21_r2_contracts.rs #1 | 0.31 | 0.61 | 0.3 |
| tests/c21_r2_events.rs #1 | 0.54 | 0.47 | -0.07 |
| tests/c21_r2_geometry.rs #1 | 33.44 | 43.09 | 9.65 |
| tests/c21_r3_contracts.rs #1 | 6.37 | 8.26 | 1.89 |
| tests/c21_r3_geometry.rs #1 | 3.53 | 4.14 | 0.61 |
| tests/c21_r3_never_block.rs #1 | 1.87 | 2.57 | 0.7 |
| tests/c21_r4_closure.rs #1 | 0.0 | 0.01 | 0.01 |
| tests/c21_replay.rs #1 | 0.33 | 0.53 | 0.2 |
| tests/c21_worked_examples.rs #1 | 0.98 | 1.19 | 0.21 |
| tests/c21_worst_case.rs #1 | 0.05 | 0.07 | 0.02 |
| tests/c22_bridges.rs #1 | 0.53 | 0.66 | 0.13 |
| tests/c22_determinism.rs #1 | 3.55 | 4.82 | 1.27 |
| tests/c22_exact_overlap.rs #1 | 0.17 | 0.24 | 0.07 |
| tests/c22_planar.rs #1 | 1.57 | 1.88 | 0.31 |
| tests/c22_planar.rs #2 | 0.38 | 0.48 | 0.1 |
| tests/c22_properties.rs #1 | 1.83 | 2.34 | 0.51 |
| tests/c22_r1_cache.rs #1 | 0.57 | 0.76 | 0.19 |
| tests/c22_r1_commands.rs #1 | 0.34 | 0.51 | 0.17 |
| tests/c22_r1_geometry.rs #1 | 0.01 | 0.01 | 0.0 |
| tests/c22_r1_surface.rs #1 | 0.24 | 0.36 | 0.12 |
| tests/c22_r2_commands.rs #1 | 0.96 | 0.99 | 0.03 |
| tests/c22_r2_geometry.rs #1 | 0.01 | 0.01 | 0.0 |
| tests/c22_r2_surface.rs #1 | 0.31 | 0.45 | 0.14 |
| tests/c22_r3_cache.rs #1 | 26.8 | 31.0 | 4.2 |
| tests/c22_r3_commands.rs #1 | 0.39 | 0.6 | 0.21 |
| tests/c22_r3_geometry.rs #1 | 0.02 | 0.03 | 0.01 |
| tests/c22_r3_surface.rs #1 | 0.47 | 0.56 | 0.09 |
| tests/c22_r4_closure.rs #1 | 0.26 | 0.36 | 0.1 |
| tests/c22_r4_closure.rs #2 | 0.0 | 0.0 | 0.0 |
| tests/c22_recovery.rs #1 | 1.28 | 1.53 | 0.25 |
| tests/c22_replay.rs #1 | 0.37 | 0.51 | 0.14 |
| tests/c22_settle.rs #1 | 0.01 | 0.02 | 0.01 |
| tests/c22_worst_case.rs #1 | 0.0 | 0.0 | 0.0 |
| tests/c23_colour.rs #1 | 0.28 | 0.45 | 0.17 |
| tests/c23_contracts.rs #1 | 3.44 | 4.64 | 1.2 |
| tests/c23_cut.rs #1 | 0.23 | 0.16 | -0.07 |
| tests/c23_layers.rs #1 | 0.33 | 0.5 | 0.17 |
| tests/c23_measure.rs #1 | 0.0 | 0.0 | 0.0 |
| tests/c23_predicates.rs #1 | 0.0 | 0.0 | 0.0 |
| tests/c23_r1_colour.rs #1 | 0.21 | 0.33 | 0.12 |
| tests/c23_r1_commands.rs #1 | 0.24 | 0.38 | 0.14 |
| tests/c23_r1_contracts.rs #1 | 0.71 | 1.04 | 0.33 |
| tests/c23_r1_controls.rs #1 | 0.28 | 0.45 | 0.17 |
| tests/c23_r1_geometry.rs #1 | 0.02 | 0.03 | 0.01 |
| tests/c23_r1_measure.rs #1 | 0.0 | 0.0 | 0.0 |
| tests/c23_r1_texture.rs #1 | 0.0 | 0.01 | 0.01 |
| tests/c23_r2_closure.rs #1 | 0.37 | 0.59 | 0.22 |
| tests/c23_r2_limits.rs #1 | 2.34 | 3.05 | 0.71 |
| tests/c23_r2_predicates.rs #1 | 0.0 | 0.0 | 0.0 |
| tests/c23_r2_smooth.rs #1 | 6.49 | 8.41 | 1.92 |
| tests/c23_r2_texture.rs #1 | 0.01 | 0.01 | 0.0 |
| tests/c23_r2_timeout.rs #1 | 1.26 | 1.38 | 0.12 |
| tests/c23_r2_watchdog.rs #1 | 2.5 | 2.61 | 0.11 |
| tests/c23_r3_budgets.rs #1 | 6.29 | 6.43 | 0.14 |
| tests/c23_r3_cancel.rs #1 | 0.0 | 0.01 | 0.01 |
| tests/c23_r3_cancellation.rs #1 | 3.1 | 3.22 | 0.12 |
| tests/c23_r3_closure.rs #1 | 0.24 | 0.41 | 0.17 |
| tests/c23_r3_inputs.rs #1 | 0.0 | 0.0 | 0.0 |
| tests/c23_r3_profile.rs #1 | 2.09 | 2.34 | 0.25 |
| tests/c23_r3_selection_cancel.rs #1 | 0.5 | 0.61 | 0.11 |
| tests/c23_r3_session.rs #1 | 0.24 | 0.38 | 0.14 |
| tests/c23_r3_texture.rs #1 | 2.09 | 2.25 | 0.16 |
| tests/c23_r3_topology.rs #1 | 0.35 | 0.51 | 0.16 |
| tests/c23_r4_closure.rs #1 | 2.37 | 2.62 | 0.25 |
| tests/c23_render.rs #1 | 0.34 | 0.52 | 0.18 |
| tests/c23_round2.rs #1 | 0.25 | 0.44 | 0.19 |
| tests/c23_round3.rs #1 | 7.41 | 9.92 | 2.51 |
| tests/c23_select.rs #1 | 0.23 | 0.38 | 0.15 |
| tests/c23_strokeops.rs #1 | 0.41 | 0.62 | 0.21 |
| tests/c23_transform.rs #1 | 0.28 | 0.44 | 0.16 |
| tests/c23_watchdog.rs #1 | 1.61 | 1.76 | 0.15 |
| tests/c24_aggregate.rs #1 | 0.2 | 0.31 | 0.11 |
| tests/c24_budgets.rs #1 | 0.0 | 0.0 | 0.0 |
| tests/c24_centre.rs #1 | 0.01 | 0.01 | 0.0 |
| tests/c24_centre_line.rs #1 | 4.99 | 5.88 | 0.89 |
| tests/c24_contracts.rs #1 | 1.47 | 1.56 | 0.09 |
| tests/c24_cutter_snapping.rs #1 | 0.2 | 0.31 | 0.11 |
| tests/c24_decode_fuzz.rs #1 | 0.04 | 0.05 | 0.01 |
| tests/c24_flatten_phases.rs #1 | 0.0 | 0.0 | 0.0 |
| tests/c24_image.rs #1 | 0.0 | 0.01 | 0.01 |
| tests/c24_measure.rs #1 | 0.0 | 0.0 | 0.0 |
| tests/c24_noding_regressions.rs #1 | 0.0 | 0.0 | 0.0 |
| tests/c24_oracle_routes.rs #1 | 0.28 | 0.43 | 0.15 |
| tests/c24_properties.rs #1 | 0.01 | 0.01 | 0.0 |
| tests/c24_r1_decoder.rs #1 | 0.21 | 0.24 | 0.03 |
| tests/c24_r1_geometry.rs #1 | 0.01 | 0.01 | 0.0 |
| tests/c24_r1_lifecycle.rs #1 | 0.31 | 0.41 | 0.1 |
| tests/c24_r1_properties.rs #1 | 0.01 | 0.01 | 0.0 |
| tests/c24_r1_topology.rs #1 | 0.0 | 0.0 | 0.0 |
| tests/c24_r2_noding.rs #1 | 0.0 | 0.0 | 0.0 |
| tests/c24_r2_properties.rs #1 | 0.01 | 0.01 | 0.0 |
| tests/c24_r2_routes.rs #1 | 0.21 | 0.36 | 0.15 |
| tests/c24_r2_topology.rs #1 | 0.2 | 0.32 | 0.12 |
| tests/c24_r2_variants.rs #1 | 0.27 | 0.44 | 0.17 |
| tests/c24_r3_decoder.rs #1 | 0.25 | 0.31 | 0.06 |
| tests/c24_r3_domains.rs #1 | 0.31 | 0.46 | 0.15 |
| tests/c24_r3_noding.rs #1 | 0.26 | 0.35 | 0.09 |
| tests/c24_r3_properties.rs #1 | 0.01 | 0.01 | 0.0 |
| tests/c24_r3_routes.rs #1 | 0.21 | 0.34 | 0.13 |
| tests/c24_r3_topology.rs #1 | 0.2 | 0.31 | 0.11 |
| tests/c24_r4_closure.rs #1 | 1.63 | 2.09 | 0.46 |
| tests/c24_route_domains.rs #1 | 0.2 | 0.34 | 0.14 |
| tests/c24_symmetry.rs #1 | 0.87 | 1.16 | 0.29 |
| tests/c24_text_layout.rs #1 | 0.28 | 0.43 | 0.15 |
| tests/c24_topology.rs #1 | 0.0 | 0.0 | 0.0 |
| tests/c24_trace.rs #1 | 0.04 | 0.05 | 0.01 |
| tests/c24_vectorize.rs #1 | 1.53 | 1.79 | 0.26 |
| tests/c24_vectorize_unit.rs #1 | 0.0 | 0.0 | 0.0 |
| tests/c31_bank_xsheet.rs #1 | 0.22 | 0.37 | 0.15 |
| tests/c31_budgets.rs #1 | 0.0 | 0.0 | 0.0 |
| tests/c31_cells.rs #1 | 0.32 | 0.5 | 0.18 |
| tests/c31_elements.rs #1 | 0.31 | 0.61 | 0.3 |
| tests/c31_exposure.rs #1 | 0.04 | 0.05 | 0.01 |
| tests/c31_obligations.rs #1 | 5.27 | 6.18 | 0.91 |
| tests/c31_r1_availability.rs #1 | 0.16 | 0.27 | 0.11 |
| tests/c31_r1_contract.rs #1 | 0.34 | 0.52 | 0.18 |
| tests/c31_r1_copy_settle.rs #1 | 0.18 | 0.29 | 0.11 |
| tests/c31_r1_hierarchy.rs #1 | 0.19 | 0.27 | 0.08 |
| tests/c31_r1_model_diff.rs #1 | 0.5 | 0.6 | 0.1 |
| tests/c31_r1_mutations.rs #1 | 3.96 | 5.38 | 1.42 |
| tests/c31_r1_notes.rs #1 | 0.32 | 0.47 | 0.15 |
| tests/c31_r1_oracle.rs #1 | 0.11 | 0.16 | 0.05 |
| tests/c31_r1_protocol.rs #1 | 0.26 | 0.36 | 0.1 |
| tests/c31_r1_selection_budget.rs #1 | 0.28 | 0.47 | 0.19 |
| tests/c31_r2_cancellation.rs #1 | 0.81 | 0.92 | 0.11 |
| tests/c31_r2_notes_boundary.rs #1 | 0.09 | 0.28 | 0.19 |
| tests/c31_r2_persistence.rs #1 | 0.38 | 0.51 | 0.13 |
| tests/c31_r2_protocol.rs #1 | 0.42 | 0.6 | 0.18 |
| tests/c31_r2_variants.rs #1 | 0.02 | 0.02 | 0.0 |
| tests/c31_r2_variants.rs #2 | 1.0 | 1.33 | 0.33 |
| tests/c31_r3_binary.rs #1 | 0.74 | 0.78 | 0.04 |
| tests/c31_r3_closure.rs #1 | 0.35 | 0.47 | 0.12 |
| tests/c31_r3_delegation.rs #1 | 1.53 | 2.0 | 0.47 |
| tests/c31_r3_hierarchy.rs #1 | 38.39 | 51.88 | 13.49 |
| tests/c31_r3_patch.rs #1 | 0.25 | 0.37 | 0.12 |
| tests/c31_r3_selection.rs #1 | 0.97 | 1.05 | 0.08 |
| tests/c31_r3_sequences.rs #1 | 0.0 | 0.0 | 0.0 |
| tests/c31_r4_closure.rs #1 | 0.77 | 1.05 | 0.28 |
| tests/c31_round1.rs #1 | 0.67 | 0.86 | 0.19 |
| tests/c31_round1_protocol.rs #1 | 0.26 | 0.38 | 0.12 |
| tests/c31_round2.rs #1 | 0.33 | 0.61 | 0.28 |
| tests/c31_round3.rs #1 | 0.35 | 0.48 | 0.13 |
| tests/c32_budgets.rs #1 | 0.0 | 0.0 | 0.0 |
| tests/c32_contract.rs #1 | 2.58 | 2.88 | 0.3 |
| tests/c32_maths.rs #1 | 0.01 | 0.01 | 0.0 |
| tests/c32_passage.rs #1 | 4.72 | 6.61 | 1.89 |
| tests/c32_protocol.rs #1 | 0.55 | 0.77 | 0.22 |
| tests/c32_r1_affine.rs #1 | 0.0 | 0.0 | 0.0 |
| tests/c32_r1_closed_forms.rs #1 | 0.42 | 0.56 | 0.14 |
| tests/c32_r1_corpus.rs #1 | 0.23 | 0.29 | 0.06 |
| tests/c32_r1_default_frame.rs #1 | 0.0 | 0.0 | 0.0 |
| tests/c32_r1_delegation.rs #1 | 0.18 | 0.29 | 0.11 |
| tests/c32_r1_passage.rs #1 | 0.0 | 0.0 | 0.0 |
| tests/c32_r1_queries.rs #1 | 0.19 | 0.13 | -0.06 |
| tests/c32_r2_c21_golden.rs #1 | 0.47 | 0.65 | 0.18 |
| tests/c32_r2_c22_golden.rs #1 | 3.48 | 4.72 | 1.24 |
| tests/c32_r2_camera.rs #1 | 0.0 | 0.0 | 0.0 |
| tests/c32_r2_camera.rs #2 | 0.18 | 0.28 | 0.1 |
| tests/c32_r2_corpus.rs #1 | 0.23 | 0.29 | 0.06 |
| tests/c32_r2_events.rs #1 | 0.22 | 0.34 | 0.12 |
| tests/c32_r2_finite.rs #1 | 0.55 | 0.75 | 0.2 |
| tests/c32_r2_numeric.rs #1 | 0.17 | 0.26 | 0.09 |
| tests/c32_r2_numeric.rs #2 | 0.0 | 0.0 | 0.0 |
| tests/c32_r2_passage.rs #1 | 0.29 | 0.41 | 0.12 |
| tests/c32_r2_path_budget.rs #1 | 7.83 | 8.21 | 0.38 |
| tests/c32_r2_tcb.rs #1 | 0.37 | 0.5 | 0.13 |
| tests/c32_r3_camera.rs #1 | 0.18 | 0.29 | 0.11 |
| tests/c32_r3_history.rs #1 | 0.27 | 0.42 | 0.15 |
| tests/c32_r4_closure.rs #1 | 45.58 | 47.35 | 1.77 |
| tests/c32_r4_delivery.rs #1 | 11.45 | 13.13 | 1.68 |
| tests/c32_readiness.rs #1 | 0.25 | 0.38 | 0.13 |
| tests/c32_round1.rs #1 | 0.0 | 0.0 | 0.0 |
| tests/c32_round1.rs #2 | 0.58 | 0.79 | 0.21 |
| tests/c32_round2.rs #1 | 0.47 | 0.62 | 0.15 |
| tests/c32_round3.rs #1 | 41.48 | 42.19 | 0.71 |
| tests/c32_round3_protocol.rs #1 | 11.03 | 12.36 | 1.33 |
| tests/c32_semantics.rs #1 | 0.48 | 0.7 | 0.22 |
| tests/c33_bitmap.rs #1 | 0.44 | 0.63 | 0.19 |
| tests/c33_goldens.rs #1 | 0.56 | 0.72 | 0.16 |
| tests/c33_inspect.rs #1 | 0.5 | 0.68 | 0.18 |
| tests/c33_layer.rs #1 | 0.01 | 0.01 | 0.0 |
| tests/c33_protocol.rs #1 | 0.28 | 0.23 | -0.05 |
| tests/c33_r1_bitmap.rs #1 | 1.48 | 1.9 | 0.42 |
| tests/c33_r1_cancel.rs #1 | 0.0 | 0.0 | 0.0 |
| tests/c33_r1_contract.rs #1 | 0.57 | 0.71 | 0.14 |
| tests/c33_r1_crash.rs #1 | 0.0 | 0.0 | 0.0 |
| tests/c33_r1_limits.rs #1 | 0.17 | 0.26 | 0.09 |
| tests/c33_r1_pixels.rs #1 | 0.38 | 0.54 | 0.16 |
| tests/c33_r1_tools.rs #1 | 0.27 | 0.37 | 0.1 |
| tests/c33_r2_cancel.rs #1 | 0.0 | 0.01 | 0.01 |
| tests/c33_r2_depth.rs #1 | 42.87 | 53.75 | 10.88 |
| tests/c33_r2_recovery.rs #1 | 0.2 | 0.34 | 0.14 |
| tests/c33_r2_stress.rs #1 | 26.42 | 32.08 | 5.66 |
| tests/c33_r2_visibility.rs #1 | 0.2 | 0.32 | 0.12 |
| tests/c33_r2_wait.rs #1 | 0.19 | 0.85 | 0.66 |
| tests/c33_r3_jobs.rs #1 | absent | 0.38 | new/absent |
| tests/c33_r3_live.rs #1 | absent | 0.57 | new/absent |
| tests/c33_r3_recovery.rs #1 | absent | 0.33 | new/absent |
| tests/c33_r3_transaction.rs #1 | absent | 0.4 | new/absent |
| tests/c33_range.rs #1 | 0.9 | 1.06 | 0.16 |
| tests/c33_render.rs #1 | 1.68 | 2.21 | 0.53 |
| tests/c33_render_list.rs #1 | 0.3 | 0.47 | 0.17 |
| tests/c33_round1.rs #1 | 1.01 | 1.2 | 0.19 |
| tests/c33_round2.rs #1 | 2.67 | 2.81 | 0.14 |
| tests/c33_round3.rs #1 | absent | 1.54 | new/absent |
| tests/c33_round3.rs #2 | absent | 0.36 | new/absent |
| tests/cancel_phases.rs #1 | 40.2 | 38.22 | -1.98 |
| tests/cli.rs #1 | 0.12 | 0.14 | 0.02 |
| tests/compile_fail.rs #1 | 0.0 | 0.0 | 0.0 |
| tests/conformance.rs #1 | 0.15 | 0.21 | 0.06 |
| tests/contracts.rs #1 | 1.68 | 1.88 | 0.2 |
| tests/declared.rs #1 | 0.3 | 0.43 | 0.13 |
| tests/frontend_schemas.rs #1 | 0.17 | 0.25 | 0.08 |
| tests/guard_fixtures.rs #1 | 3.74 | 3.52 | -0.22 |
| tests/hostile.rs #1 | 2.67 | 3.19 | 0.52 |
| tests/inventory.rs #1 | 0.01 | 0.01 | 0.0 |
| tests/journal.rs #1 | 4.27 | 5.71 | 1.44 |
| tests/mcp.rs #1 | 0.37 | 0.49 | 0.12 |
| tests/migrations.rs #1 | 0.0 | 0.0 | 0.0 |
| tests/never_block.rs #1 | 1.52 | 2.01 | 0.49 |
| tests/properties.rs #1 | 0.16 | 0.22 | 0.06 |
| tests/properties.rs #2 | 1.37 | 1.77 | 0.4 |
| tests/protocol.rs #1 | 6.03 | 6.23 | 0.2 |
| tests/round_trip.rs #1 | 1.33 | 1.33 | 0.0 |
| tests/staged_commit.rs #1 | 8.04 | 10.79 | 2.75 |
| tests/staged_inverse.rs #1 | 5.5 | 7.51 | 2.01 |
| tests/wire_corpus.rs #1 | 0.0 | 0.01 | 0.01 |
| tests/zip64.rs #1 | 1.19 | 1.59 | 0.4 |
| unittests src/lib.rs #1 | 0.0 | 0.0 | 0.0 |
| unittests src/lib.rs #10 | 0.11 | 0.04 | -0.07 |
| unittests src/lib.rs #11 | 0.13 | 0.15 | 0.02 |
| unittests src/lib.rs #12 | 0.0 | 0.15 | 0.15 |
| unittests src/lib.rs #13 | 0.0 | 0.01 | 0.01 |
| unittests src/lib.rs #14 | 0.0 | 0.0 | 0.0 |
| unittests src/lib.rs #15 | 0.06 | 0.01 | -0.05 |
| unittests src/lib.rs #16 | 0.61 | 0.08 | -0.53 |
| unittests src/lib.rs #17 | absent | 0.78 | new/absent |
| unittests src/lib.rs #2 | 0.0 | 0.0 | 0.0 |
| unittests src/lib.rs #3 | 0.62 | 0.0 | -0.62 |
| unittests src/lib.rs #4 | 0.19 | 0.81 | 0.62 |
| unittests src/lib.rs #5 | 1.64 | 0.37 | -1.27 |
| unittests src/lib.rs #6 | 0.0 | 1.89 | 1.89 |
| unittests src/lib.rs #7 | 0.0 | 0.0 | 0.0 |
| unittests src/lib.rs #8 | 0.0 | 0.0 | 0.0 |
| unittests src/lib.rs #9 | 0.02 | 0.0 | -0.02 |
| unittests src/main.rs #1 | 0.0 | 0.0 | 0.0 |
| unittests src/main.rs #2 | 0.18 | 0.3 | 0.12 |
| unittests src/main.rs #3 | 0.0 | 0.0 | 0.0 |
