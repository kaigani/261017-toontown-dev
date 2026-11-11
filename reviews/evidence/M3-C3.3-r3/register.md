# C3.3 round-three register

181 implemented / 2 divergent / 14 deferred-with-owner / 1 closed / 0 missing / 0 pending.

| Unit | Requirement | Status | Evidence |
|---|---|---|---|
| RF-001 | camera composition | implemented | c33_render; c33_goldens; c33_r1_pixels |
| RF-002 | element subset composition | implemented | c33_render; c33_goldens; c33_r1_pixels |
| RF-003 | drawing-space composition | implemented | c33_render; c33_goldens; c33_r1_pixels |
| RF-004 | scene/frame addressing | implemented | c33_render; c33_goldens; c33_r1_pixels |
| RF-005 | camera override | implemented | c33_render; c33_goldens; c33_r1_pixels |
| RF-006 | width and height | implemented | c33_render; c33_goldens; c33_r1_pixels |
| RF-007 | aspect-preserving single dimension | implemented | c33_render; c33_goldens; c33_r1_pixels |
| RF-008 | scale | implemented | c33_render; c33_goldens; c33_r1_pixels |
| RF-009 | scene background | implemented | c33_render; c33_goldens; c33_r1_pixels |
| RF-010 | transparent background | implemented | c33_render; c33_goldens; c33_r1_pixels |
| RF-011 | explicit colour background | implemented | c33_render; c33_goldens; c33_r1_pixels |
| RF-012 | PNG output | implemented | c33_render; c33_goldens; c33_r1_pixels |
| RF-013 | JPEG output | implemented | c33_render; c33_goldens; c33_r1_pixels |
| RF-014 | JPEG quality | implemented | c33_render; c33_goldens; c33_r1_pixels |
| RF-015 | inline base64 | implemented | c33_render; c33_goldens; c33_r1_pixels |
| RF-016 | absolute file output | implemented | c33_render; c33_goldens; c33_r1_pixels |
| RF-017 | fail on conflict | implemented | c33_render; c33_goldens; c33_r1_pixels |
| RF-018 | overwrite on conflict | implemented | c33_render; c33_goldens; c33_r1_pixels |
| RF-019 | rename on conflict | implemented | c33_render; c33_goldens; c33_r1_pixels |
| RF-020 | file-access policy | implemented | c33_render; c33_goldens; c33_r1_pixels |
| RF-021 | width/height/bytes/SHA/revision metadata | implemented | c33_render; c33_goldens; c33_r1_pixels |
| RF-022 | repeat PNG identity | implemented | c33_render; c33_goldens; c33_r1_pixels |
| RF-023 | history re-render identity | implemented | c33_render; c33_goldens; c33_r1_pixels |
| RF-024 | save/open re-render identity | implemented | c33_render; c33_goldens; c33_r1_pixels |
| RF-025 | staged batch render | implemented | c33_render; c33_goldens; c33_r1_pixels |
| RF-026 | failed batch rollback | implemented | c33_render; c33_goldens; c33_r1_pixels |
| RF-027 | overlays excluded | implemented | c33_render; c33_goldens; c33_r1_pixels |
| LIMIT-028 | inline long-side limit value | implemented | c33_render::sizes_scale_and_limits; render.rs |
| LIMIT-029 | inline byte limit value | implemented | c33_render::sizes_scale_and_limits; render.rs |
| LIMIT-030 | max output side | implemented | c33_render::sizes_scale_and_limits; render.rs |
| LIMIT-031 | supported formats | implemented | c33_render::sizes_scale_and_limits; render.rs |
| LIMIT-032 | oversized side refusal | implemented | c33_render::sizes_scale_and_limits; render.rs |
| LIMIT-033 | inline refusal code | implemented | c33_render::sizes_scale_and_limits; render.rs |
| LIST-034 | evaluated scene input | implemented | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-035 | memoized ancestor evaluation | implemented | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-036 | drawing-to-pixel affine | implemented | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-037 | three projected points | implemented | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-038 | drawing bounds | implemented | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-039 | camera pixel bounds | implemented | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-040 | normal world-Z ordering | implemented | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-041 | exact-depth Timeline ordering | implemented | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-042 | background role tier | implemented | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-043 | foreground role tier | implemented | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-044 | envelope as normal | implemented | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-045 | hidden element | implemented | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-046 | hidden ancestor | implemented | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-047 | annotation omitted | implemented | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-048 | empty exposure | implemented | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-049 | empty geometry | implemented | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-050 | out-of-frame verdict | implemented | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-051 | near-plane culling | implemented | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-052 | background exposure extension | implemented | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-053 | mirrored scale | implemented | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-054 | skew | implemented | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-055 | nested parent transforms | implemented | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-056 | pivots | implemented | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-057 | camera roll | implemented | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-058 | dynamic FOV | implemented | c33_render_list; c33_r1_pixels; render_list.rs |
| BITMAP-059 | pixel-space origin/y direction | implemented | c33_bitmap; draw_render.rs; inspect.rs |
| BITMAP-060 | stored affine honored | implemented | c33_bitmap; draw_render.rs; inspect.rs |
| BITMAP-061 | native-size scale | implemented | c33_bitmap; draw_render.rs; inspect.rs |
| BITMAP-062 | one source pixel per output pixel | implemented | c33_bitmap; draw_render.rs; inspect.rs |
| BITMAP-063 | straight alpha import | implemented | c33_bitmap; draw_render.rs; inspect.rs |
| BITMAP-064 | full source resolution | implemented | c33_bitmap; draw_render.rs; inspect.rs |
| BITMAP-065 | bounds agree with render | implemented | c33_bitmap; draw_render.rs; inspect.rs |
| BITMAP-066 | transparent pixels not hit | implemented | c33_bitmap; draw_render.rs; inspect.rs |
| DESCRIBE-067 | same render-list input | implemented | c33_inspect; c33_r1_tools; inspect.rs |
| DESCRIBE-068 | draw order | implemented | c33_inspect; c33_r1_tools; inspect.rs |
| DESCRIBE-069 | drawing IDs | implemented | c33_inspect; c33_r1_tools; inspect.rs |
| DESCRIBE-070 | world transform | implemented | c33_inspect; c33_r1_tools; inspect.rs |
| DESCRIBE-071 | normalized bounds | implemented | c33_inspect; c33_r1_tools; inspect.rs |
| DESCRIBE-072 | field bounds | implemented | c33_inspect; c33_r1_tools; inspect.rs |
| DESCRIBE-073 | pixel bounds | implemented | c33_inspect; c33_r1_tools; inspect.rs |
| DESCRIBE-074 | opacity | implemented | c33_inspect; c33_r1_tools; inspect.rs |
| DESCRIBE-075 | roles | implemented | c33_inspect; c33_r1_tools; inspect.rs |
| DESCRIBE-076 | omitted reasons | implemented | c33_inspect; c33_r1_tools; inspect.rs |
| DESCRIBE-077 | empty-exposure warning | implemented | c33_inspect; c33_r1_tools; inspect.rs |
| DESCRIBE-078 | revision | implemented | c33_inspect; c33_r1_tools; inspect.rs |
| HIT-079 | image space | implemented | c33_inspect; c33_bitmap; c33_r1_tools |
| HIT-080 | camera space | implemented | c33_inspect; c33_bitmap; c33_r1_tools |
| HIT-081 | scene space | implemented | c33_inspect; c33_bitmap; c33_r1_tools |
| HIT-082 | top-most first | implemented | c33_inspect; c33_bitmap; c33_r1_tools |
| HIT-083 | mirrored geometry | implemented | c33_inspect; c33_bitmap; c33_r1_tools |
| HIT-084 | overlapping nodes | implemented | c33_inspect; c33_bitmap; c33_r1_tools |
| HIT-085 | depth/stacking | implemented | c33_inspect; c33_bitmap; c33_r1_tools |
| HIT-086 | drawing/layer/object/colour IDs | implemented | c33_inspect; c33_bitmap; c33_r1_tools |
| HIT-087 | transparent bitmap exclusion | implemented | c33_inspect; c33_bitmap; c33_r1_tools |
| HIT-088 | drawing without context refusal | implemented | c33_inspect; c33_bitmap; c33_r1_tools |
| CONVERT-089 | drawing space | implemented | c33_inspect; c33_r1_tools; inspect.rs |
| CONVERT-090 | local space | implemented | c33_inspect; c33_r1_tools; inspect.rs |
| CONVERT-091 | parent space | implemented | c33_inspect; c33_r1_tools; inspect.rs |
| CONVERT-092 | scene/world/field aliases | implemented | c33_inspect; c33_r1_tools; inspect.rs |
| CONVERT-093 | camera space | implemented | c33_inspect; c33_r1_tools; inspect.rs |
| CONVERT-094 | normalized camera space | implemented | c33_inspect; c33_r1_tools; inspect.rs |
| CONVERT-095 | image space | implemented | c33_inspect; c33_r1_tools; inspect.rs |
| CONVERT-096 | forward mapping | implemented | c33_inspect; c33_r1_tools; inspect.rs |
| CONVERT-097 | inverse round trip | implemented | c33_inspect; c33_r1_tools; inspect.rs |
| CONVERT-098 | 16:9 corners | implemented | c33_inspect; c33_r1_tools; inspect.rs |
| CONVERT-099 | z-plane unprojection | implemented | c33_inspect; c33_r1_tools; inspect.rs |
| CONVERT-100 | near-plane refusal | implemented | c33_inspect; c33_r1_tools; inspect.rs |
| CONVERT-101 | element context required | implemented | c33_inspect; c33_r1_tools; inspect.rs |
| CONVERT-102 | finite point validation | implemented | c33_inspect; c33_r1_tools; inspect.rs |
| SHEET-103 | explicit frame list | implemented | c33_inspect; c33_goldens; c33_r1_tools |
| SHEET-104 | stepped range | implemented | c33_inspect; c33_goldens; c33_r1_tools |
| SHEET-105 | default ceil sqrt columns | implemented | c33_inspect; c33_goldens; c33_r1_tools |
| SHEET-106 | explicit columns | implemented | c33_inspect; c33_goldens; c33_r1_tools |
| SHEET-107 | aspect-derived cell height | implemented | c33_inspect; c33_goldens; c33_r1_tools |
| SHEET-108 | cell-width setting | implemented | c33_inspect; c33_goldens; c33_r1_tools |
| SHEET-109 | deterministic digit labels | implemented | c33_inspect; c33_goldens; c33_r1_tools |
| SHEET-110 | label toggle | implemented | c33_inspect; c33_goldens; c33_r1_tools |
| SHEET-111 | cell pixels equal frame pixels | implemented | c33_inspect; c33_goldens; c33_r1_tools |
| SHEET-112 | cell coordinates | implemented | c33_inspect; c33_goldens; c33_r1_tools |
| SHEET-113 | bytes/SHA/revision | implemented | c33_inspect; c33_goldens; c33_r1_tools |
| SHEET-114 | side-limit refusal | implemented | c33_inspect; c33_goldens; c33_r1_tools |
| SHEET-115 | inline-limit refusal | implemented | c33_inspect; c33_goldens; c33_r1_tools |
| RANGE-116 | snapshot-owned document | implemented | c33_range; render.rs; c33_r1_contract |
| RANGE-117 | inclusive range | implemented | c33_range; render.rs; c33_r1_contract |
| RANGE-118 | step | implemented | c33_range; render.rs; c33_r1_contract |
| RANGE-119 | name token | implemented | c33_range; render.rs; c33_r1_contract |
| RANGE-120 | scene token | implemented | c33_range; render.rs; c33_r1_contract |
| RANGE-121 | frame token | implemented | c33_range; render.rs; c33_r1_contract |
| RANGE-122 | zero padding | implemented | c33_range; render.rs; c33_r1_contract |
| RANGE-123 | bad pattern refusal | implemented | c33_range; render.rs; c33_r1_contract |
| RANGE-124 | duplicate name refusal | implemented | c33_range; render.rs; c33_r1_contract |
| RANGE-125 | absolute directory | implemented | c33_range; render.rs; c33_r1_contract |
| RANGE-126 | createDirs | implemented | c33_range; render.rs; c33_r1_contract |
| RANGE-127 | missing-dir refusal | implemented | c33_range; render.rs; c33_r1_contract |
| RANGE-128 | conflict fail | implemented | c33_range; render.rs; c33_r1_contract |
| RANGE-129 | conflict overwrite | implemented | c33_range; render.rs; c33_r1_contract |
| RANGE-130 | conflict rename | implemented | c33_range; render.rs; c33_r1_contract |
| RANGE-131 | PNG format | implemented | c33_range; render.rs; c33_r1_contract |
| RANGE-132 | JPEG format | implemented | c33_range; render.rs; c33_r1_contract |
| RANGE-133 | BMP format | implemented | c33_range; render.rs; c33_r1_contract |
| RANGE-134 | TGA format | implemented | c33_range; render.rs; c33_r1_contract |
| RANGE-135 | lossless frame equivalence | implemented | c33_range; render.rs; c33_r1_contract |
| RANGE-136 | companion manifest | implemented | c33_range; render.rs; c33_r1_contract |
| RANGE-137 | manifest roles and hashes | implemented | c33_range; render.rs; c33_r1_contract |
| RANGE-138 | per-frame progress | implemented | c33_range; render.rs; c33_r1_contract |
| RANGE-139 | job cancellation | implemented | c33_range; render.rs; c33_r1_contract |
| RANGE-140 | staging cleanup on cancellation | implemented | c33_range; render.rs; c33_r1_contract |
| RANGE-141 | request ownership | implemented | c33_range; render.rs; c33_r1_contract |
| RANGE-142 | concurrent edit isolation | implemented | c33_range; render.rs; c33_r1_contract |
| COMMON-143 | query document-revision purity | implemented | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-144 | query history purity | implemented | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-145 | query selection/frame purity | implemented | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-146 | query event silence | implemented | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-147 | schema validation | implemented | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-148 | executed registry examples | implemented | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-149 | declaration freshness | implemented | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-150 | IPC freshness | implemented | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-151 | work-account charging | implemented | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-152 | request cancellation | implemented | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-153 | invalidation range keyed edits | implemented | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-154 | all-frame invalidation forms | implemented | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-155 | inherited reviewer ownership | implemented | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-156 | M2 goldens unchanged | implemented | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-157 | fixed raster configuration | implemented | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-158 | cross-OS gate bytes | implemented | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-159 | debug golden bytes | implemented | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-160 | release gate bytes | implemented | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-161 | headless SDK conformance | implemented | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-162 | test-hooks dev-only | implemented | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-163 | allow inventory 97/40/0 | implemented | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-164 | no new forbidden product constructs | implemented | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-165 | dependency audit | implemented | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-166 | 28 CI jobs green | implemented | c33_render; c33_r1_contract; CI 42795220374 |
| C33-R1-01 | sub-microfield depth tie | implemented | c33_r2_depth::seeded_anchor_groups_4096; six controlled members, document-tree Timeline oracle, hit-order and perturbation samples |
| C33-R1-02 | wait true final result | implemented | unchanged c33_r1_contract manifest probe; c33_round1 timeout/addressability; c33_r2_wait responsiveness and timeout/cancel assertions pass before the separate warning failure |
| C33-R1-03 | failed sequence atomic publication | implemented | unchanged failed-publication probe; c33_r2_stress::last_target_readonly_preserves_originals; builder original restoration |
| C33-R1-04 | known oversized inline refusal before work | implemented | c33_r2_wait::dimension_preflight_precedes_slow_work (frame/sheet/range); ::encoded_byte_limit_is_checked_after_encoding; unchanged c33_r1_limits |
| C33-R1-05 | generic render include/exclude override | implemented | c33_r2_visibility command parity, unknown refs, hidden ancestor and precedence; unchanged c33_r1_tools generic override probe |
| C33-R1-06 | generic render timing metadata | implemented | unchanged c33_r1_tools timing probe; c33_r2_visibility renderMs; recorded example normalization source adjudicated |
| DEF-173 | attached in-app render parity | deferred-with-owner | M3-C3.3-handoff.md |
| DEF-174 | activeView composition | deferred-with-owner | M3-C3.3-handoff.md |
| DEF-175 | interactive draft quality | deferred-with-owner | M3-C3.3-handoff.md |
| DEF-176 | view coordinate space | deferred-with-owner | M3-C3.3-handoff.md |
| DEF-177 | view pick radius/handles | deferred-with-owner | M3-C3.3-handoff.md |
| DEF-178 | xsheet composition | deferred-with-owner | M3-C3.3-handoff.md |
| DEF-179 | bone/envelope deformation | deferred-with-owner | M3-C3.3-handoff.md |
| DEF-180 | path/function/velocity IPC final shape | deferred-with-owner | M3-C3.3-handoff.md |
| DEF-181 | effect scopes/particles/filter options/cache | deferred-with-owner | M3-C3.3-handoff.md |
| DEF-182 | sounds and mouth shapes | deferred-with-owner | M3-C3.3-handoff.md |
| DEF-183 | native-size importer writing placement | deferred-with-owner | M3-C3.3-handoff.md |
| DEF-184 | E1 saved-project replay | deferred-with-owner | M3-C3.3-handoff.md |
| DEF-185 | C22-T2/R1-12 hardening | deferred-with-owner | M3-C3.3-handoff.md |
| C33-R1-07 | cancellation for an empty render list | implemented | c33_r2_cancel::cancellation_at_every_empty_and_nonempty_boundary; unchanged c33_r1_cancel and c23_r3_cancel |
| C33-R1-08 | process-death staging cleanup | divergent | Positive ownership and warning retention verified; typed cancellation error absent: c33_r3_recovery::cancelled_record_keeps_typed_error. R2-01 closed; R2-02 partially closed. |
| REG-render.frame.run | registry row render.frame.run | implemented | c33_render, c33_r1_pixels/bitmap/limits; c33_r2_visibility, c33_r2_wait limit/byte probes; golden equality |
| REG-render.limits.get | registry row render.limits.get | implemented | named command suites and reviewer probes |
| REG-render.range.run | registry row render.range.run | divergent | Positive ownership and warning retention verified; typed cancellation error absent: c33_r3_recovery::cancelled_record_keeps_typed_error. R2-01 closed; R2-02 partially closed. |
| REG-inspect.frame.describe | registry row inspect.frame.describe | implemented | c33_r2_depth anchor oracle; c33_r2_visibility parity; unchanged inspection geometry suites |
| REG-inspect.hitTest | registry row inspect.hitTest | implemented | c33_r2_depth reverse-order samples; unchanged c33_r1_pixels and c33_inspect geometry |
| REG-inspect.coordinates.convert | registry row inspect.coordinates.convert | implemented | named command suites and reviewer probes |
| REG-inspect.contactSheet.render | registry row inspect.contactSheet.render | implemented | c33_r2_visibility cell-byte parity; c33_r2_recovery evaluation counts; work-delay dimension preflight |
| READINESS-FRACTIONAL | fractional-frame coordinate conversion acceptance probe | closed | Integer registry ruling SI-109; unchanged typed-refusal probe |
| READINESS-EVALUATION-COUNT | instrumented count of evaluations in an actual render-list command | implemented | c33_round1 measured describe/render/hit per-node counts; c33_r2_recovery contact-sheet N-frame count (serialized recorder) |
| SPEC-WIDTH-DEFAULT | disposition of conflicting generic render width defaults | implemented | SI-110 ruling; generic and command use camera width |
| DEF-THUMBNAILS | project and drawing thumbnails | deferred-with-owner | explicit owner in derived.rs and traceability |
