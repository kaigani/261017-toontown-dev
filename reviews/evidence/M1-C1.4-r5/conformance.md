# Final scoped conformance register

Product `e2eba79`; r4 unit boundaries and accepted dispositions retained. Only S11-L81 promotes for C14-R4-01. S11-L96 (SI-049) remains the sole divergence.

Implemented / divergent / missing / deferred-with-owner: requirements **153 / 1 / 0 / 72**; declarations **21 / 0 / 0 / 11**.

## requirements

| Unit | Status | Evidence and owner disposition |
|---|---|---|
| S11-L19 | deferred-with-owner | Builder: GUI parity/gestures M4, render and scene determinism M3; kernel determinism already tested. Explicit implementing owner checkpoints: builder M3 C3.3, M4 C4.2. |
| S11-L20 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L21 | deferred-with-owner | Builder: GUI parity/gestures M4, render and scene determinism M3; kernel determinism already tested. Explicit implementing owner checkpoints: builder M3 C3.3, M4 C4.2. |
| S11-L22 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L23 | implemented | Round-one fix verified on 39ea1b2; proving tests listed. |
| S11-L24 | deferred-with-owner | Builder: GUI parity/gestures M4, render and scene determinism M3; kernel determinism already tested. Explicit implementing owner checkpoints: builder M3 C3.3, M4 C4.2. |
| S11-L25 | deferred-with-owner | Builder: GUI parity/gestures M4, render and scene determinism M3; kernel determinism already tested. Explicit implementing owner checkpoints: builder M3 C3.3, M4 C4.2. |
| S11-L26 | implemented | The original audit/replay/cancellation divergence closes on 5c46aa6. Commit facts, exact atomic replay/history and terminal cancellation pass the unchanged r3 suites and scoped r4 probes. The remaining group expectRev omission is registered at S11-L81. |
| S11-L27 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L57 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L62 | deferred-with-owner | Builder: M4 GUI binding and M3 rendering (plan §4). Explicit implementing owner checkpoints: builder M3 C3.3, M4 C4.2. |
| S11-L66 | implemented | Round-one fix verified on 39ea1b2; proving tests listed. |
| S11-L69 | deferred-with-owner | Builder: M4 GUI binding and M3 rendering (plan §4). Explicit implementing owner checkpoints: builder M3 C3.3, M4 C4.2. |
| S11-L79 | deferred-with-owner | Builder M4: ADR-0015 §10, attached HTTP and independent sessions. Explicit implementing owner checkpoints: builder M4 C4.1. |
| S11-L80 | deferred-with-owner | Headless kernel works now; builder M3 renderer and M4 virtual UI/input remain. Explicit implementing owner checkpoints: builder M3 C3.3, M4 C4.2. |
| S11-L81 | implemented | C14-R4-01 closes on b16a39a/e2eba79: matching and stale atomic group expectRev, single revision/history, sequential step guards, export and exact replay, stale replay and malformed guard refusal independently checked. |
| S11-L84 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L86 | deferred-with-owner | Builder M4: ADR-0015 §10, attached HTTP and independent sessions. Explicit implementing owner checkpoints: builder M4 C4.1. |
| S11-L88 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L96 | divergent | SI-049 remains the original spec-owner divergence: implements 2025-06-18/2025-11-25; current-stable target awaits a spec-owner decision. SDK 24/24 does not resolve it. |
| S11-L102 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L103 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L104 | deferred-with-owner | Builder M4, ADR-0015 §10: localhost, Origin, bearer auth and discovery are unimplemented. Explicit implementing owner checkpoints: builder M4 C4.2. |
| S11-L107 | deferred-with-owner | Builder M4, ADR-0015 §10: localhost, Origin, bearer auth and discovery are unimplemented. Explicit implementing owner checkpoints: builder M4 C4.2. |
| S11-L111 | implemented | Explicit --mcp-stdio flag; no desktop MCP listener in this checkpoint. |
| S11-L113 | implemented | Metadata under _meta.toontown; instructions exactly match app://docs/agent-guide. |
| S11-L117 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L126 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L129 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L133 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L135 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L145 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L146 | deferred-with-owner | Builder M3: actual transform conversion, timecode and render coordinate chains; docs enumerate the conventions now. Explicit implementing owner checkpoints: builder M3 C3.3. |
| S11-L147 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L148 | deferred-with-owner | Builder M3: actual transform conversion, timecode and render coordinate chains; docs enumerate the conventions now. Explicit implementing owner checkpoints: builder M3 C3.3. |
| S11-L149 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L150 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L154 | deferred-with-owner | Builder M3: actual transform conversion, timecode and render coordinate chains; docs enumerate the conventions now. Explicit implementing owner checkpoints: builder M3 C3.3. |
| S11-L158 | deferred-with-owner | Builder M3: actual transform conversion, timecode and render coordinate chains; docs enumerate the conventions now. Explicit implementing owner checkpoints: builder M3 C3.3. |
| S11-L159 | deferred-with-owner | Builder M3: actual transform conversion, timecode and render coordinate chains; docs enumerate the conventions now. Explicit implementing owner checkpoints: builder M3 C3.3. |
| S11-L160 | deferred-with-owner | Builder M3: actual transform conversion, timecode and render coordinate chains; docs enumerate the conventions now. Explicit implementing owner checkpoints: builder M3 C3.3. |
| S11-L161 | deferred-with-owner | Builder M3: actual transform conversion, timecode and render coordinate chains; docs enumerate the conventions now. Explicit implementing owner checkpoints: builder M3 C3.3. |
| S11-L162 | deferred-with-owner | Builder M3: actual transform conversion, timecode and render coordinate chains; docs enumerate the conventions now. Explicit implementing owner checkpoints: builder M3 C3.3. |
| S11-L164 | deferred-with-owner | Builder M3: actual transform conversion, timecode and render coordinate chains; docs enumerate the conventions now. Explicit implementing owner checkpoints: builder M3 C3.3. |
| S11-L173 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L177 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L178 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L179 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L180 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L181 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L182 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L183 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L184 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L185 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L186 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L187 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L189 | implemented | Round-one fix verified on 39ea1b2; proving tests listed. |
| S11-L194 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L195 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L196 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L198 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L203 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L206 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L207 | implemented | Round-one fix verified on 39ea1b2; proving tests listed. |
| S11-L211 | deferred-with-owner | Builder M2/M8/M10 media ingestion; existing doc.import.json respects roots and scopes. Explicit implementing owner checkpoints: builder M2 C2.3, M8 C8.2, M10 C10.2. |
| S11-L216 | implemented | Round-two fix verified on 39ea1b2 by unchanged reviewer suites; staged effects and external-change confirmation are implemented. |
| S11-L219 | implemented | The original audit/replay/cancellation divergence closes on 5c46aa6. Commit facts, exact atomic replay/history and terminal cancellation pass the unchanged r3 suites and scoped r4 probes. The remaining group expectRev omission is registered at S11-L81. |
| S11-L222 | implemented | Fallback tokens are single-use, session/params/revision bound, five-minute TTL; attached askInApp portion deferred to builder M4. |
| S11-L227 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L229 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L234 | deferred-with-owner | Builder M4 attached coexistence/indicator; programmatic pause/resume/disconnect controls exist and are tested now. Explicit implementing owner checkpoints: builder M4 C4.1. |
| S11-L237 | deferred-with-owner | Builder M4 attached coexistence/indicator; programmatic pause/resume/disconnect controls exist and are tested now. Explicit implementing owner checkpoints: builder M4 C4.1. |
| S11-L240 | deferred-with-owner | Builder M4 attached coexistence/indicator; programmatic pause/resume/disconnect controls exist and are tested now. Explicit implementing owner checkpoints: builder M4 C4.1. |
| S11-L242 | deferred-with-owner | Builder M4 attached coexistence/indicator; programmatic pause/resume/disconnect controls exist and are tested now. Explicit implementing owner checkpoints: builder M4 C4.1. |
| S11-L243 | deferred-with-owner | Builder M4 attached coexistence/indicator; programmatic pause/resume/disconnect controls exist and are tested now. Explicit implementing owner checkpoints: builder M4 C4.1. |
| S11-L249 | implemented | Round-one fix verified on 39ea1b2; proving tests listed. |
| S11-L257 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L263 | implemented | Round-one fix verified on 39ea1b2; proving tests listed. Render annotations remain builder M3. |
| S11-L271 | implemented | Round-one fix verified on 39ea1b2; proving tests listed. |
| S11-L272 | implemented | Round-one fix verified on 39ea1b2; proving tests listed. |
| S11-L273 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L274 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L280 | implemented | Round-one fix verified on 39ea1b2; proving tests listed. |
| S11-L282 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L297 | implemented | Round-one fix verified on 39ea1b2; proving tests listed. |
| S11-L300 | implemented | Round-one fix verified on 39ea1b2; proving tests listed. |
| S11-L302 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L309 | implemented | Round-one fix verified on 39ea1b2; proving tests listed. |
| S11-L311 | implemented | Round-two fix verified on 39ea1b2 by unchanged reviewer suites; staged effects and external-change confirmation are implemented. |
| S11-L313 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L314 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L315 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L321 | implemented | doc.* convenience mappings; registry validates the retained command parameters. C14-R1-01 covers wrapper safety-option validation. |
| S11-L322 | implemented | doc.* convenience mappings; registry validates the retained command parameters. C14-R1-01 covers wrapper safety-option validation. |
| S11-L323 | implemented | doc.* convenience mappings; registry validates the retained command parameters. C14-R1-01 covers wrapper safety-option validation. |
| S11-L324 | implemented | doc.* convenience mappings; registry validates the retained command parameters. C14-R1-01 covers wrapper safety-option validation. |
| S11-L326 | implemented | doc.* convenience mappings; registry validates the retained command parameters. C14-R1-01 covers wrapper safety-option validation. |
| S11-L332 | deferred-with-owner | Builder M2 drawing renders/gestures; M3 scene rendering and coordinates; M4 UI inspection and input, as declared. Explicit implementing owner checkpoints: builder M2 C2.3, M3 C3.3, M4 C4.2. |
| S11-L333 | deferred-with-owner | Builder M2 drawing renders/gestures; M3 scene rendering and coordinates; M4 UI inspection and input, as declared. Explicit implementing owner checkpoints: builder M2 C2.3, M3 C3.3, M4 C4.2. |
| S11-L334 | deferred-with-owner | Builder M2 drawing renders/gestures; M3 scene rendering and coordinates; M4 UI inspection and input, as declared. Explicit implementing owner checkpoints: builder M2 C2.3, M3 C3.3, M4 C4.2. |
| S11-L335 | deferred-with-owner | Builder M2 drawing renders/gestures; M3 scene rendering and coordinates; M4 UI inspection and input, as declared. Explicit implementing owner checkpoints: builder M2 C2.3, M3 C3.3, M4 C4.2. |
| S11-L336 | deferred-with-owner | Builder M2 drawing renders/gestures; M3 scene rendering and coordinates; M4 UI inspection and input, as declared. Explicit implementing owner checkpoints: builder M2 C2.3, M3 C3.3, M4 C4.2. |
| S11-L337 | deferred-with-owner | Builder M2 drawing renders/gestures; M3 scene rendering and coordinates; M4 UI inspection and input, as declared. Explicit implementing owner checkpoints: builder M2 C2.3, M3 C3.3, M4 C4.2. |
| S11-L338 | deferred-with-owner | Builder M2 drawing renders/gestures; M3 scene rendering and coordinates; M4 UI inspection and input, as declared. Explicit implementing owner checkpoints: builder M2 C2.3, M3 C3.3, M4 C4.2. |
| S11-L339 | deferred-with-owner | Builder M2 drawing renders/gestures; M3 scene rendering and coordinates; M4 UI inspection and input, as declared. Explicit implementing owner checkpoints: builder M2 C2.3, M3 C3.3, M4 C4.2. |
| S11-L341 | deferred-with-owner | Builder M2 drawing renders/gestures; M3 scene rendering and coordinates; M4 UI inspection and input, as declared. Explicit implementing owner checkpoints: builder M2 C2.3, M3 C3.3, M4 C4.2. |
| S11-L344 | deferred-with-owner | Builder M2 drawing renders/gestures; M3 scene rendering and coordinates; M4 UI inspection and input, as declared. Explicit implementing owner checkpoints: builder M2 C2.3, M3 C3.3, M4 C4.2. |
| S11-L346 | deferred-with-owner | Builder M2 drawing renders/gestures; M3 scene rendering and coordinates; M4 UI inspection and input, as declared. Explicit implementing owner checkpoints: builder M2 C2.3, M3 C3.3, M4 C4.2. |
| S11-L350 | deferred-with-owner | Builder M2 drawing renders/gestures; M3 scene rendering and coordinates; M4 UI inspection and input, as declared. Explicit implementing owner checkpoints: builder M2 C2.3, M3 C3.3, M4 C4.2. |
| S11-L354 | deferred-with-owner | Builder M2 drawing renders/gestures; M3 scene rendering and coordinates; M4 UI inspection and input, as declared. Explicit implementing owner checkpoints: builder M2 C2.3, M3 C3.3, M4 C4.2. |
| S11-L355 | deferred-with-owner | Builder M2 drawing renders/gestures; M3 scene rendering and coordinates; M4 UI inspection and input, as declared. Explicit implementing owner checkpoints: builder M2 C2.3, M3 C3.3, M4 C4.2. |
| S11-L356 | deferred-with-owner | Builder M2 drawing renders/gestures; M3 scene rendering and coordinates; M4 UI inspection and input, as declared. Explicit implementing owner checkpoints: builder M2 C2.3, M3 C3.3, M4 C4.2. |
| S11-L359 | deferred-with-owner | Builder M2 drawing renders/gestures; M3 scene rendering and coordinates; M4 UI inspection and input, as declared. Explicit implementing owner checkpoints: builder M2 C2.3, M3 C3.3, M4 C4.2. |
| S11-L360 | deferred-with-owner | Builder M2 drawing renders/gestures; M3 scene rendering and coordinates; M4 UI inspection and input, as declared. Explicit implementing owner checkpoints: builder M2 C2.3, M3 C3.3, M4 C4.2. |
| S11-L363 | deferred-with-owner | Builder M2 drawing renders/gestures; M3 scene rendering and coordinates; M4 UI inspection and input, as declared. Explicit implementing owner checkpoints: builder M2 C2.3, M3 C3.3, M4 C4.2. |
| S11-L365 | deferred-with-owner | Builder M2 drawing renders/gestures; M3 scene rendering and coordinates; M4 UI inspection and input, as declared. Explicit implementing owner checkpoints: builder M2 C2.3, M3 C3.3, M4 C4.2. |
| S11-L372 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L373 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L374 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L375 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L376 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L377 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L378 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L382 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L386 | implemented | Round-one fix verified on 39ea1b2; proving tests listed. |
| S11-L387 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L388 | deferred-with-owner | Builder M2/M3/M4 according to mapped command ownership; wrappers for rendering still need full parameter parity. Explicit implementing owner checkpoints: builder M2 C2.3, M3 C3.3, M4 C4.2. |
| S11-L389 | deferred-with-owner | Builder M2/M3/M4 according to mapped command ownership; wrappers for rendering still need full parameter parity. Explicit implementing owner checkpoints: builder M2 C2.3, M3 C3.3, M4 C4.2. |
| S11-L390 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L391 | deferred-with-owner | Builder M2/M3/M4 according to mapped command ownership; wrappers for rendering still need full parameter parity. Explicit implementing owner checkpoints: builder M2 C2.3, M3 C3.3, M4 C4.2. |
| S11-L392 | deferred-with-owner | Builder M2/M3/M4 according to mapped command ownership; wrappers for rendering still need full parameter parity. Explicit implementing owner checkpoints: builder M2 C2.3, M3 C3.3, M4 C4.2. |
| S11-L393 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L394 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L395 | deferred-with-owner | Builder M2/M3/M4 according to mapped command ownership; wrappers for rendering still need full parameter parity. Explicit implementing owner checkpoints: builder M2 C2.3, M3 C3.3, M4 C4.2. |
| S11-L397 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L404 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L407 | implemented | Round-one fix verified on 39ea1b2; proving tests listed. |
| S11-L410 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L412 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L415 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L417 | deferred-with-owner | C14-R1-16 ticket: builder M4 C4.x (plan section 4); ignored failing test, body unchanged. Explicit implementing owner checkpoints: builder M4 C4.2. |
| S11-L420 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L431 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L432 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L433 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L434 | deferred-with-owner | Builder M3 render/xsheet commands; template is discoverable now. Explicit implementing owner checkpoints: builder M3 C3.3. |
| S11-L435 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L436 | deferred-with-owner | Builder M3 render/xsheet commands; template is discoverable now. Explicit implementing owner checkpoints: builder M3 C3.3. |
| S11-L437 | deferred-with-owner | Builder M3 render/xsheet commands; template is discoverable now. Explicit implementing owner checkpoints: builder M3 C3.3. |
| S11-L438 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L439 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L440 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L441 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L442 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L443 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L444 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L446 | deferred-with-owner | Builder M3 render/xsheet commands; template is discoverable now. Explicit implementing owner checkpoints: builder M3 C3.3. |
| S11-L451 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L457 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L458 | implemented | Round-one fix verified on 39ea1b2; proving tests listed. |
| S11-L459 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L460 | deferred-with-owner | Builder M3/M4/M9/area milestone: specific event producers are not all wired; app.state.changed exists now. Explicit implementing owner checkpoints: builder M3 C3.3, M4 C4.2, M9 C9.1. |
| S11-L461 | deferred-with-owner | Builder M3/M4/M9/area milestone: specific event producers are not all wired; app.state.changed exists now. Explicit implementing owner checkpoints: builder M3 C3.3, M4 C4.2, M9 C9.1. |
| S11-L462 | deferred-with-owner | Builder M3/M4/M9/area milestone: specific event producers are not all wired; app.state.changed exists now. Explicit implementing owner checkpoints: builder M3 C3.3, M4 C4.2, M9 C9.1. |
| S11-L463 | deferred-with-owner | Builder M3/M4/M9/area milestone: specific event producers are not all wired; app.state.changed exists now. Explicit implementing owner checkpoints: builder M3 C3.3, M4 C4.2, M9 C9.1. |
| S11-L464 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L465 | deferred-with-owner | Builder M3/M4/M9/area milestone: specific event producers are not all wired; app.state.changed exists now. Explicit implementing owner checkpoints: builder M3 C3.3, M4 C4.2, M9 C9.1. |
| S11-L466 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L467 | deferred-with-owner | Builder M3/M4/M9/area milestone: specific event producers are not all wired; app.state.changed exists now. Explicit implementing owner checkpoints: builder M3 C3.3, M4 C4.2, M9 C9.1. |
| S11-L469 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L476 | implemented | Protocol numeric JSON-RPC codes and documented command dotted codes; inherited extra codes are adjudicated in SI-020, SI-046 and SI-048. See report inventory. |
| S11-L480 | implemented | Protocol numeric JSON-RPC codes and documented command dotted codes; inherited extra codes are adjudicated in SI-020, SI-046 and SI-048. See report inventory. |
| S11-L481 | implemented | Protocol numeric JSON-RPC codes and documented command dotted codes; inherited extra codes are adjudicated in SI-020, SI-046 and SI-048. See report inventory. |
| S11-L482 | implemented | Protocol numeric JSON-RPC codes and documented command dotted codes; inherited extra codes are adjudicated in SI-020, SI-046 and SI-048. See report inventory. |
| S11-L483 | implemented | Protocol numeric JSON-RPC codes and documented command dotted codes; inherited extra codes are adjudicated in SI-020, SI-046 and SI-048. See report inventory. |
| S11-L484 | implemented | Protocol numeric JSON-RPC codes and documented command dotted codes; inherited extra codes are adjudicated in SI-020, SI-046 and SI-048. See report inventory. |
| S11-L485 | implemented | Protocol numeric JSON-RPC codes and documented command dotted codes; inherited extra codes are adjudicated in SI-020, SI-046 and SI-048. See report inventory. |
| S11-L486 | implemented | Protocol numeric JSON-RPC codes and documented command dotted codes; inherited extra codes are adjudicated in SI-020, SI-046 and SI-048. See report inventory. |
| S11-L487 | implemented | Protocol numeric JSON-RPC codes and documented command dotted codes; inherited extra codes are adjudicated in SI-020, SI-046 and SI-048. See report inventory. |
| S11-L488 | implemented | Protocol numeric JSON-RPC codes and documented command dotted codes; inherited extra codes are adjudicated in SI-020, SI-046 and SI-048. See report inventory. |
| S11-L489 | implemented | Protocol numeric JSON-RPC codes and documented command dotted codes; inherited extra codes are adjudicated in SI-020, SI-046 and SI-048. See report inventory. |
| S11-L490 | implemented | Protocol numeric JSON-RPC codes and documented command dotted codes; inherited extra codes are adjudicated in SI-020, SI-046 and SI-048. See report inventory. |
| S11-L491 | implemented | Protocol numeric JSON-RPC codes and documented command dotted codes; inherited extra codes are adjudicated in SI-020, SI-046 and SI-048. See report inventory. |
| S11-L492 | implemented | Protocol numeric JSON-RPC codes and documented command dotted codes; inherited extra codes are adjudicated in SI-020, SI-046 and SI-048. See report inventory. |
| S11-L493 | implemented | Protocol numeric JSON-RPC codes and documented command dotted codes; inherited extra codes are adjudicated in SI-020, SI-046 and SI-048. See report inventory. |
| S11-L494 | implemented | Protocol numeric JSON-RPC codes and documented command dotted codes; inherited extra codes are adjudicated in SI-020, SI-046 and SI-048. See report inventory. |
| S11-L495 | implemented | Protocol numeric JSON-RPC codes and documented command dotted codes; inherited extra codes are adjudicated in SI-020, SI-046 and SI-048. See report inventory. |
| S11-L497 | implemented | Protocol numeric JSON-RPC codes and documented command dotted codes; inherited extra codes are adjudicated in SI-020, SI-046 and SI-048. See report inventory. |
| S11-L505 | implemented | The original audit/replay/cancellation divergence closes on 5c46aa6. Commit facts, exact atomic replay/history and terminal cancellation pass the unchanged r3 suites and scoped r4 probes. The remaining group expectRev omission is registered at S11-L81. |
| S11-L508 | deferred-with-owner | Builder M4 live panel. Existing batch export is divergent (C14-R1-15: drops batches, turns dry runs into edits). Explicit implementing owner checkpoints: builder M4 C4.2. |
| S11-L511 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L518 | implemented | All ten prompts and arguments exist; test verifies referenced ids. Text is hand-authored with a checked command list, not mechanically generated; domain procedures remain future milestones. |
| S11-L524 | implemented | All ten prompts and arguments exist; test verifies referenced ids. Text is hand-authored with a checked command list, not mechanically generated; domain procedures remain future milestones. |
| S11-L525 | implemented | All ten prompts and arguments exist; test verifies referenced ids. Text is hand-authored with a checked command list, not mechanically generated; domain procedures remain future milestones. |
| S11-L526 | implemented | All ten prompts and arguments exist; test verifies referenced ids. Text is hand-authored with a checked command list, not mechanically generated; domain procedures remain future milestones. |
| S11-L527 | implemented | All ten prompts and arguments exist; test verifies referenced ids. Text is hand-authored with a checked command list, not mechanically generated; domain procedures remain future milestones. |
| S11-L528 | implemented | All ten prompts and arguments exist; test verifies referenced ids. Text is hand-authored with a checked command list, not mechanically generated; domain procedures remain future milestones. |
| S11-L529 | implemented | All ten prompts and arguments exist; test verifies referenced ids. Text is hand-authored with a checked command list, not mechanically generated; domain procedures remain future milestones. |
| S11-L530 | implemented | All ten prompts and arguments exist; test verifies referenced ids. Text is hand-authored with a checked command list, not mechanically generated; domain procedures remain future milestones. |
| S11-L531 | implemented | All ten prompts and arguments exist; test verifies referenced ids. Text is hand-authored with a checked command list, not mechanically generated; domain procedures remain future milestones. |
| S11-L532 | implemented | All ten prompts and arguments exist; test verifies referenced ids. Text is hand-authored with a checked command list, not mechanically generated; domain procedures remain future milestones. |
| S11-L533 | implemented | All ten prompts and arguments exist; test verifies referenced ids. Text is hand-authored with a checked command list, not mechanically generated; domain procedures remain future milestones. |
| S11-L535 | implemented | All ten prompts and arguments exist; test verifies referenced ids. Text is hand-authored with a checked command list, not mechanically generated; domain procedures remain future milestones. |
| S11-L541 | implemented | Projection/pagination through doc.* plus get_state/list_commands; see 01 for malformed wrapper page/options. |
| S11-L543 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L545 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L546 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L548 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L549 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L550 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L552 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L560 | deferred-with-owner | Registry bindings/agentOnly gate exists now; exhaustive real GUI action coverage is builder M4. Explicit implementing owner checkpoints: builder M4 C4.2. |
| S11-L564 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L566 | deferred-with-owner | M1 kernel roundtrips covered; create-via-domain-command for every type follows implementing milestones. Explicit implementing owner checkpoints: builder M2 C2.1, M3 C3.1, M3 C3.2, M7 C7.2, M8 C8.1, M9 C9.1. |
| S11-L568 | deferred-with-owner | Builder: M2 E2/gestures; M3/M10 E1/render; M6 E4; M8 E3; M9 E5; M4 E6 (plan §4). Explicit implementing owner checkpoints: builder M2 C2.3, M3 C3.3, M4 C4.2, M6 C6.2, M8 C8.2, M9 C9.1, M10 C10.2. |
| S11-L569 | deferred-with-owner | Builder: M2 E2/gestures; M3/M10 E1/render; M6 E4; M8 E3; M9 E5; M4 E6 (plan §4). Explicit implementing owner checkpoints: builder M2 C2.3, M3 C3.3, M4 C4.2, M6 C6.2, M8 C8.2, M9 C9.1, M10 C10.2. |
| S11-L571 | implemented | Round-one fix verified on 39ea1b2; proving tests listed. Document-thread restart remains builder M4; a never-returning handler still occupies its worker. |
| S11-L573 | implemented | The original audit/replay/cancellation divergence closes on 5c46aa6. Commit facts, exact atomic replay/history and terminal cancellation pass the unchanged r3 suites and scoped r4 probes. The remaining group expectRev omission is registered at S11-L81. |
| S11-L575 | implemented | Within the implemented M1 kernel; domain operations keep their declared later owners. |
| S11-L576 | deferred-with-owner | Builder: M2 E2/gestures; M3/M10 E1/render; M6 E4; M8 E3; M9 E5; M4 E6 (plan §4). Explicit implementing owner checkpoints: builder M2 C2.3, M3 C3.3, M4 C4.2, M6 C6.2, M8 C8.2, M9 C9.1, M10 C10.2. |
| S11-L578 | deferred-with-owner | Builder: M2 E2/gestures; M3/M10 E1/render; M6 E4; M8 E3; M9 E5; M4 E6 (plan §4). Explicit implementing owner checkpoints: builder M2 C2.3, M3 C3.3, M4 C4.2, M6 C6.2, M8 C8.2, M9 C9.1, M10 C10.2. |
| S11-L580 | deferred-with-owner | Builder: M2 E2/gestures; M3/M10 E1/render; M6 E4; M8 E3; M9 E5; M4 E6 (plan §4). Explicit implementing owner checkpoints: builder M2 C2.3, M3 C3.3, M4 C4.2, M6 C6.2, M8 C8.2, M9 C9.1, M10 C10.2. |
| S11-L582 | deferred-with-owner | Builder: M2 E2/gestures; M3/M10 E1/render; M6 E4; M8 E3; M9 E5; M4 E6 (plan §4). Explicit implementing owner checkpoints: builder M2 C2.3, M3 C3.3, M4 C4.2, M6 C6.2, M8 C8.2, M9 C9.1, M10 C10.2. |
| S11-L584 | deferred-with-owner | Builder: M2 E2/gestures; M3/M10 E1/render; M6 E4; M8 E3; M9 E5; M4 E6 (plan §4). Explicit implementing owner checkpoints: builder M2 C2.3, M3 C3.3, M4 C4.2, M6 C6.2, M8 C8.2, M9 C9.1, M10 C10.2. |
| S11-L586 | deferred-with-owner | Builder: M2 E2/gestures; M3/M10 E1/render; M6 E4; M8 E3; M9 E5; M4 E6 (plan §4). Explicit implementing owner checkpoints: builder M2 C2.3, M3 C3.3, M4 C4.2, M6 C6.2, M8 C8.2, M9 C9.1, M10 C10.2. |
| S11-L588 | implemented | Round-one fix verified on 39ea1b2; proving tests listed. Network half remains builder at the first network command. |

## declarations

| Unit | Status | Evidence and owner disposition |
|---|---|---|
| app.info.get | implemented | Typed params/result schema, scopes, kind, undo metadata and example assembled; M1 examples execute with mapped session/token/path fixture. Event rows are non-invocable. |
| agent.activity.export | implemented | Original divergence closes: committed edits survive publication errors; atomic groups preserve exact revisions and undo history. Group expectRev omission belongs to script execution row S11-L81. |
| agent.activity.list | implemented | Typed params/result schema, scopes, kind, undo metadata and example assembled; M1 examples execute with mapped session/token/path fixture. Event rows are non-invocable. |
| agent.activity.persist.set | implemented | Typed params/result schema, scopes, kind, undo metadata and example assembled; M1 examples execute with mapped session/token/path fixture. Event rows are non-invocable. |
| agent.confirmation.requested | deferred-with-owner | Builder M4: HTTP/attached user confirmation remains deferred; implemented in-memory parts do not complete this declaration's contract. Explicit implementing owner checkpoints: builder M4 C4.1. |
| agent.confirmation.respond | deferred-with-owner | Builder M4: HTTP/attached user confirmation remains deferred; implemented in-memory parts do not complete this declaration's contract. Explicit implementing owner checkpoints: builder M4 C4.1. |
| agent.policy.defaults.set | implemented | Defaults rehydrated; downgrade/restart and corrupt-file variants checked. |
| agent.server.set | deferred-with-owner | Builder M4: HTTP/attached user confirmation remains deferred; implemented in-memory parts do not complete this declaration's contract. Explicit implementing owner checkpoints: builder M4 C4.1. |
| agent.server.status | deferred-with-owner | Builder M4: HTTP/attached user confirmation remains deferred; implemented in-memory parts do not complete this declaration's contract. Explicit implementing owner checkpoints: builder M4 C4.1. |
| agent.session.connected | implemented | Typed params/result schema, scopes, kind, undo metadata and example assembled; M1 examples execute with mapped session/token/path fixture. Event rows are non-invocable. |
| agent.session.disconnect | implemented | Typed params/result schema, scopes, kind, undo metadata and example assembled; M1 examples execute with mapped session/token/path fixture. Event rows are non-invocable. |
| agent.session.disconnected | implemented | Typed params/result schema, scopes, kind, undo metadata and example assembled; M1 examples execute with mapped session/token/path fixture. Event rows are non-invocable. |
| agent.session.grant | implemented | Typed params/result schema, scopes, kind, undo metadata and example assembled; M1 examples execute with mapped session/token/path fixture. Event rows are non-invocable. |
| agent.session.pause | implemented | Typed params/result schema, scopes, kind, undo metadata and example assembled; M1 examples execute with mapped session/token/path fixture. Event rows are non-invocable. |
| agent.session.resume | implemented | Typed params/result schema, scopes, kind, undo metadata and example assembled; M1 examples execute with mapped session/token/path fixture. Event rows are non-invocable. |
| agent.token.regenerate | deferred-with-owner | Builder M4: HTTP/attached user confirmation remains deferred; implemented in-memory parts do not complete this declaration's contract. Explicit implementing owner checkpoints: builder M4 C4.1. |
| agent.typedTools.set | implemented | Typed params/result schema, scopes, kind, undo metadata and example assembled; M1 examples execute with mapped session/token/path fixture. Event rows are non-invocable. |
| inspect.contactSheet.render | deferred-with-owner | Builder M4 C4.2 for UI tree/screenshot; M3 for frame/hit/coordinates/contact sheet. Typed declaration and notImplemented response are present. Explicit implementing owner checkpoints: builder M3 C3.3, M4 C4.2. |
| inspect.coordinates.convert | deferred-with-owner | Builder M4 C4.2 for UI tree/screenshot; M3 for frame/hit/coordinates/contact sheet. Typed declaration and notImplemented response are present. Explicit implementing owner checkpoints: builder M3 C3.3, M4 C4.2. |
| inspect.frame.describe | deferred-with-owner | Builder M4 C4.2 for UI tree/screenshot; M3 for frame/hit/coordinates/contact sheet. Typed declaration and notImplemented response are present. Explicit implementing owner checkpoints: builder M3 C3.3, M4 C4.2. |
| inspect.hitTest | deferred-with-owner | Builder M4 C4.2 for UI tree/screenshot; M3 for frame/hit/coordinates/contact sheet. Typed declaration and notImplemented response are present. Explicit implementing owner checkpoints: builder M3 C3.3, M4 C4.2. |
| inspect.screenshot | deferred-with-owner | Builder M4 C4.2 for UI tree/screenshot; M3 for frame/hit/coordinates/contact sheet. Typed declaration and notImplemented response are present. Explicit implementing owner checkpoints: builder M3 C3.3, M4 C4.2. |
| inspect.ui.tree | deferred-with-owner | Builder M4 C4.2 for UI tree/screenshot; M3 for frame/hit/coordinates/contact sheet. Typed declaration and notImplemented response are present. Explicit implementing owner checkpoints: builder M3 C3.3, M4 C4.2. |
| checkpoint.create | implemented | Typed params/result schema, scopes, kind, undo metadata and example assembled; M1 examples execute with mapped session/token/path fixture. Event rows are non-invocable. |
| checkpoint.delete | implemented | Typed params/result schema, scopes, kind, undo metadata and example assembled; M1 examples execute with mapped session/token/path fixture. Event rows are non-invocable. |
| checkpoint.list | implemented | Typed params/result schema, scopes, kind, undo metadata and example assembled; M1 examples execute with mapped session/token/path fixture. Event rows are non-invocable. |
| checkpoint.restore | implemented | Typed params/result schema, scopes, kind, undo metadata and example assembled; M1 examples execute with mapped session/token/path fixture. Event rows are non-invocable. |
| job.cancel | implemented | Typed params/result schema, scopes, kind, undo metadata and example assembled; M1 examples execute with mapped session/token/path fixture. Event rows are non-invocable. |
| job.finished | implemented | Typed params/result schema, scopes, kind, undo metadata and example assembled; M1 examples execute with mapped session/token/path fixture. Event rows are non-invocable. |
| job.list | implemented | Typed params/result schema, scopes, kind, undo metadata and example assembled; M1 examples execute with mapped session/token/path fixture. Event rows are non-invocable. |
| job.progress | implemented | Typed params/result schema, scopes, kind, undo metadata and example assembled; M1 examples execute with mapped session/token/path fixture. Event rows are non-invocable. |
| job.wait | implemented | Typed params/result schema, scopes, kind, undo metadata and example assembled; M1 examples execute with mapped session/token/path fixture. Event rows are non-invocable. |

