# C3.3 round-one requirement register

{"implemented": 168, "divergent": 13, "missing": 3, "deferred-with-owner": 13, "pending": 0}

Implemented entries use the named source review and executed suites; missing entries record acceptance evidence or specification disposition not established.

| ID | Requirement | Disposition | Evidence |
|---|---|---|---|
| RF-001 | camera composition | implemented  | c33_render; c33_goldens; c33_r1_pixels |
| RF-002 | element subset composition | implemented  | c33_render; c33_goldens; c33_r1_pixels |
| RF-003 | drawing-space composition | implemented  | c33_render; c33_goldens; c33_r1_pixels |
| RF-004 | scene/frame addressing | implemented  | c33_render; c33_goldens; c33_r1_pixels |
| RF-005 | camera override | implemented  | c33_render; c33_goldens; c33_r1_pixels |
| RF-006 | width and height | implemented  | c33_render; c33_goldens; c33_r1_pixels |
| RF-007 | aspect-preserving single dimension | implemented  | c33_render; c33_goldens; c33_r1_pixels |
| RF-008 | scale | implemented  | c33_render; c33_goldens; c33_r1_pixels |
| RF-009 | scene background | implemented  | c33_render; c33_goldens; c33_r1_pixels |
| RF-010 | transparent background | implemented  | c33_render; c33_goldens; c33_r1_pixels |
| RF-011 | explicit colour background | implemented  | c33_render; c33_goldens; c33_r1_pixels |
| RF-012 | PNG output | implemented  | c33_render; c33_goldens; c33_r1_pixels |
| RF-013 | JPEG output | implemented  | c33_render; c33_goldens; c33_r1_pixels |
| RF-014 | JPEG quality | implemented  | c33_render; c33_goldens; c33_r1_pixels |
| RF-015 | inline base64 | implemented  | c33_render; c33_goldens; c33_r1_pixels |
| RF-016 | absolute file output | implemented  | c33_render; c33_goldens; c33_r1_pixels |
| RF-017 | fail on conflict | implemented  | c33_render; c33_goldens; c33_r1_pixels |
| RF-018 | overwrite on conflict | implemented  | c33_render; c33_goldens; c33_r1_pixels |
| RF-019 | rename on conflict | implemented  | c33_render; c33_goldens; c33_r1_pixels |
| RF-020 | file-access policy | implemented  | c33_render; c33_goldens; c33_r1_pixels |
| RF-021 | width/height/bytes/SHA/revision metadata | implemented  | c33_render; c33_goldens; c33_r1_pixels |
| RF-022 | repeat PNG identity | implemented  | c33_render; c33_goldens; c33_r1_pixels |
| RF-023 | history re-render identity | implemented  | c33_render; c33_goldens; c33_r1_pixels |
| RF-024 | save/open re-render identity | implemented  | c33_render; c33_goldens; c33_r1_pixels |
| RF-025 | staged batch render | implemented  | c33_render; c33_goldens; c33_r1_pixels |
| RF-026 | failed batch rollback | implemented  | c33_render; c33_goldens; c33_r1_pixels |
| RF-027 | overlays excluded | implemented  | c33_render; c33_goldens; c33_r1_pixels |
| LIMIT-028 | inline long-side limit value | implemented  | c33_render::sizes_scale_and_limits; render.rs |
| LIMIT-029 | inline byte limit value | implemented  | c33_render::sizes_scale_and_limits; render.rs |
| LIMIT-030 | max output side | implemented  | c33_render::sizes_scale_and_limits; render.rs |
| LIMIT-031 | supported formats | implemented  | c33_render::sizes_scale_and_limits; render.rs |
| LIMIT-032 | oversized side refusal | implemented  | c33_render::sizes_scale_and_limits; render.rs |
| LIMIT-033 | inline refusal code | implemented  | c33_render::sizes_scale_and_limits; render.rs |
| LIST-034 | evaluated scene input | implemented  | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-035 | memoized ancestor evaluation | implemented  | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-036 | drawing-to-pixel affine | implemented  | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-037 | three projected points | implemented  | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-038 | drawing bounds | implemented  | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-039 | camera pixel bounds | implemented  | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-040 | normal world-Z ordering | implemented  | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-041 | exact-depth Timeline ordering | implemented  | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-042 | background role tier | implemented  | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-043 | foreground role tier | implemented  | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-044 | envelope as normal | implemented  | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-045 | hidden element | implemented  | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-046 | hidden ancestor | implemented  | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-047 | annotation omitted | implemented  | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-048 | empty exposure | implemented  | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-049 | empty geometry | implemented  | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-050 | out-of-frame verdict | implemented  | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-051 | near-plane culling | implemented  | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-052 | background exposure extension | implemented  | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-053 | mirrored scale | implemented  | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-054 | skew | implemented  | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-055 | nested parent transforms | implemented  | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-056 | pivots | implemented  | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-057 | camera roll | implemented  | c33_render_list; c33_r1_pixels; render_list.rs |
| LIST-058 | dynamic FOV | implemented  | c33_render_list; c33_r1_pixels; render_list.rs |
| BITMAP-059 | pixel-space origin/y direction | implemented  | c33_bitmap; draw_render.rs; inspect.rs |
| BITMAP-060 | stored affine honored | implemented  | c33_bitmap; draw_render.rs; inspect.rs |
| BITMAP-061 | native-size scale | implemented  | c33_bitmap; draw_render.rs; inspect.rs |
| BITMAP-062 | one source pixel per output pixel | implemented  | c33_bitmap; draw_render.rs; inspect.rs |
| BITMAP-063 | straight alpha import | implemented  | c33_bitmap; draw_render.rs; inspect.rs |
| BITMAP-064 | full source resolution | implemented  | c33_bitmap; draw_render.rs; inspect.rs |
| BITMAP-065 | bounds agree with render | implemented  | c33_bitmap; draw_render.rs; inspect.rs |
| BITMAP-066 | transparent pixels not hit | implemented  | c33_bitmap; draw_render.rs; inspect.rs |
| DESCRIBE-067 | same render-list input | implemented  | c33_inspect; c33_r1_tools; inspect.rs |
| DESCRIBE-068 | draw order | implemented  | c33_inspect; c33_r1_tools; inspect.rs |
| DESCRIBE-069 | drawing IDs | implemented  | c33_inspect; c33_r1_tools; inspect.rs |
| DESCRIBE-070 | world transform | implemented  | c33_inspect; c33_r1_tools; inspect.rs |
| DESCRIBE-071 | normalized bounds | implemented  | c33_inspect; c33_r1_tools; inspect.rs |
| DESCRIBE-072 | field bounds | implemented  | c33_inspect; c33_r1_tools; inspect.rs |
| DESCRIBE-073 | pixel bounds | implemented  | c33_inspect; c33_r1_tools; inspect.rs |
| DESCRIBE-074 | opacity | implemented  | c33_inspect; c33_r1_tools; inspect.rs |
| DESCRIBE-075 | roles | implemented  | c33_inspect; c33_r1_tools; inspect.rs |
| DESCRIBE-076 | omitted reasons | implemented  | c33_inspect; c33_r1_tools; inspect.rs |
| DESCRIBE-077 | empty-exposure warning | implemented  | c33_inspect; c33_r1_tools; inspect.rs |
| DESCRIBE-078 | revision | implemented  | c33_inspect; c33_r1_tools; inspect.rs |
| HIT-079 | image space | implemented  | c33_inspect; c33_bitmap; c33_r1_tools |
| HIT-080 | camera space | implemented  | c33_inspect; c33_bitmap; c33_r1_tools |
| HIT-081 | scene space | implemented  | c33_inspect; c33_bitmap; c33_r1_tools |
| HIT-082 | top-most first | implemented  | c33_inspect; c33_bitmap; c33_r1_tools |
| HIT-083 | mirrored geometry | implemented  | c33_inspect; c33_bitmap; c33_r1_tools |
| HIT-084 | overlapping nodes | implemented  | c33_inspect; c33_bitmap; c33_r1_tools |
| HIT-085 | depth/stacking | implemented  | c33_inspect; c33_bitmap; c33_r1_tools |
| HIT-086 | drawing/layer/object/colour IDs | implemented  | c33_inspect; c33_bitmap; c33_r1_tools |
| HIT-087 | transparent bitmap exclusion | implemented  | c33_inspect; c33_bitmap; c33_r1_tools |
| HIT-088 | drawing without context refusal | implemented  | c33_inspect; c33_bitmap; c33_r1_tools |
| CONVERT-089 | drawing space | implemented  | c33_inspect; c33_r1_tools; inspect.rs |
| CONVERT-090 | local space | implemented  | c33_inspect; c33_r1_tools; inspect.rs |
| CONVERT-091 | parent space | implemented  | c33_inspect; c33_r1_tools; inspect.rs |
| CONVERT-092 | scene/world/field aliases | implemented  | c33_inspect; c33_r1_tools; inspect.rs |
| CONVERT-093 | camera space | implemented  | c33_inspect; c33_r1_tools; inspect.rs |
| CONVERT-094 | normalized camera space | implemented  | c33_inspect; c33_r1_tools; inspect.rs |
| CONVERT-095 | image space | implemented  | c33_inspect; c33_r1_tools; inspect.rs |
| CONVERT-096 | forward mapping | implemented  | c33_inspect; c33_r1_tools; inspect.rs |
| CONVERT-097 | inverse round trip | implemented  | c33_inspect; c33_r1_tools; inspect.rs |
| CONVERT-098 | 16:9 corners | implemented  | c33_inspect; c33_r1_tools; inspect.rs |
| CONVERT-099 | z-plane unprojection | implemented  | c33_inspect; c33_r1_tools; inspect.rs |
| CONVERT-100 | near-plane refusal | implemented  | c33_inspect; c33_r1_tools; inspect.rs |
| CONVERT-101 | element context required | implemented  | c33_inspect; c33_r1_tools; inspect.rs |
| CONVERT-102 | finite point validation | implemented  | c33_inspect; c33_r1_tools; inspect.rs |
| SHEET-103 | explicit frame list | implemented  | c33_inspect; c33_goldens; c33_r1_tools |
| SHEET-104 | stepped range | implemented  | c33_inspect; c33_goldens; c33_r1_tools |
| SHEET-105 | default ceil sqrt columns | implemented  | c33_inspect; c33_goldens; c33_r1_tools |
| SHEET-106 | explicit columns | implemented  | c33_inspect; c33_goldens; c33_r1_tools |
| SHEET-107 | aspect-derived cell height | implemented  | c33_inspect; c33_goldens; c33_r1_tools |
| SHEET-108 | cell-width setting | implemented  | c33_inspect; c33_goldens; c33_r1_tools |
| SHEET-109 | deterministic digit labels | implemented  | c33_inspect; c33_goldens; c33_r1_tools |
| SHEET-110 | label toggle | implemented  | c33_inspect; c33_goldens; c33_r1_tools |
| SHEET-111 | cell pixels equal frame pixels | implemented  | c33_inspect; c33_goldens; c33_r1_tools |
| SHEET-112 | cell coordinates | implemented  | c33_inspect; c33_goldens; c33_r1_tools |
| SHEET-113 | bytes/SHA/revision | implemented  | c33_inspect; c33_goldens; c33_r1_tools |
| SHEET-114 | side-limit refusal | implemented  | c33_inspect; c33_goldens; c33_r1_tools |
| SHEET-115 | inline-limit refusal | implemented  | c33_inspect; c33_goldens; c33_r1_tools |
| RANGE-116 | snapshot-owned document | implemented  | c33_range; render.rs; c33_r1_contract |
| RANGE-117 | inclusive range | implemented  | c33_range; render.rs; c33_r1_contract |
| RANGE-118 | step | implemented  | c33_range; render.rs; c33_r1_contract |
| RANGE-119 | name token | implemented  | c33_range; render.rs; c33_r1_contract |
| RANGE-120 | scene token | implemented  | c33_range; render.rs; c33_r1_contract |
| RANGE-121 | frame token | implemented  | c33_range; render.rs; c33_r1_contract |
| RANGE-122 | zero padding | implemented  | c33_range; render.rs; c33_r1_contract |
| RANGE-123 | bad pattern refusal | implemented  | c33_range; render.rs; c33_r1_contract |
| RANGE-124 | duplicate name refusal | implemented  | c33_range; render.rs; c33_r1_contract |
| RANGE-125 | absolute directory | implemented  | c33_range; render.rs; c33_r1_contract |
| RANGE-126 | createDirs | implemented  | c33_range; render.rs; c33_r1_contract |
| RANGE-127 | missing-dir refusal | implemented  | c33_range; render.rs; c33_r1_contract |
| RANGE-128 | conflict fail | implemented  | c33_range; render.rs; c33_r1_contract |
| RANGE-129 | conflict overwrite | implemented  | c33_range; render.rs; c33_r1_contract |
| RANGE-130 | conflict rename | implemented  | c33_range; render.rs; c33_r1_contract |
| RANGE-131 | PNG format | implemented  | c33_range; render.rs; c33_r1_contract |
| RANGE-132 | JPEG format | implemented  | c33_range; render.rs; c33_r1_contract |
| RANGE-133 | BMP format | implemented  | c33_range; render.rs; c33_r1_contract |
| RANGE-134 | TGA format | implemented  | c33_range; render.rs; c33_r1_contract |
| RANGE-135 | lossless frame equivalence | implemented  | c33_range; render.rs; c33_r1_contract |
| RANGE-136 | companion manifest | implemented  | c33_range; render.rs; c33_r1_contract |
| RANGE-137 | manifest roles and hashes | implemented  | c33_range; render.rs; c33_r1_contract |
| RANGE-138 | per-frame progress | implemented  | c33_range; render.rs; c33_r1_contract |
| RANGE-139 | job cancellation | implemented  | c33_range; render.rs; c33_r1_contract |
| RANGE-140 | staging cleanup on cancellation | implemented  | c33_range; render.rs; c33_r1_contract |
| RANGE-141 | request ownership | implemented  | c33_range; render.rs; c33_r1_contract |
| RANGE-142 | concurrent edit isolation | implemented  | c33_range; render.rs; c33_r1_contract |
| COMMON-143 | query document-revision purity | implemented  | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-144 | query history purity | implemented  | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-145 | query selection/frame purity | implemented  | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-146 | query event silence | implemented  | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-147 | schema validation | implemented  | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-148 | executed registry examples | implemented  | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-149 | declaration freshness | implemented  | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-150 | IPC freshness | implemented  | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-151 | work-account charging | implemented  | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-152 | request cancellation | implemented  | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-153 | invalidation range keyed edits | implemented  | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-154 | all-frame invalidation forms | implemented  | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-155 | inherited reviewer ownership | implemented  | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-156 | M2 goldens unchanged | implemented  | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-157 | fixed raster configuration | implemented  | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-158 | cross-OS gate bytes | implemented  | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-159 | debug golden bytes | implemented  | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-160 | release gate bytes | implemented  | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-161 | headless SDK conformance | implemented  | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-162 | test-hooks dev-only | implemented  | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-163 | allow inventory 97/40/0 | implemented  | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-164 | no new forbidden product constructs | implemented  | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-165 | dependency audit | implemented  | c33_render; c33_r1_contract; CI 42795220374 |
| COMMON-166 | 28 CI jobs green | implemented  | c33_render; c33_r1_contract; CI 42795220374 |
| C33-R1-01 | sub-microfield depth tie | divergent  | c33_r1_contract::sub_microfield_depth_difference_uses_timeline_order |
| C33-R1-02 | wait true final result | divergent  | c33_r1_contract::wait_true_returns_completed_manifest |
| C33-R1-03 | failed sequence atomic publication | divergent  | c33_r1_contract::failed_sequence_publication_leaves_no_frames |
| C33-R1-04 | known oversized inline refusal before work | divergent  | c33_r1_limits |
| C33-R1-05 | generic render include/exclude override | divergent  | c33_r1_tools::render_frame_accepts_exclude_override |
| C33-R1-06 | generic render timing metadata | divergent  | c33_r1_tools::render_frame_reports_render_ms |
| DEF-173 | attached in-app render parity | deferred-with-owner M4 C4.2 | M3-C3.3-handoff.md |
| DEF-174 | activeView composition | deferred-with-owner M4 C4.2 | M3-C3.3-handoff.md |
| DEF-175 | interactive draft quality | deferred-with-owner M4 C4.2 | M3-C3.3-handoff.md |
| DEF-176 | view coordinate space | deferred-with-owner M4 C4.2 | M3-C3.3-handoff.md |
| DEF-177 | view pick radius/handles | deferred-with-owner M4 C4.2 | M3-C3.3-handoff.md |
| DEF-178 | xsheet composition | deferred-with-owner M5 C5.1 | M3-C3.3-handoff.md |
| DEF-179 | bone/envelope deformation | deferred-with-owner M6 C6.3 | M3-C3.3-handoff.md |
| DEF-180 | path/function/velocity IPC final shape | deferred-with-owner M6 C6.2, SI-096 | M3-C3.3-handoff.md |
| DEF-181 | effect scopes/particles/filter options/cache | deferred-with-owner M7 | M3-C3.3-handoff.md |
| DEF-182 | sounds and mouth shapes | deferred-with-owner M8 | M3-C3.3-handoff.md |
| DEF-183 | native-size importer writing placement | deferred-with-owner M10 | M3-C3.3-handoff.md |
| DEF-184 | E1 saved-project replay | deferred-with-owner C3.4 | M3-C3.3-handoff.md |
| DEF-185 | C22-T2/R1-12 hardening | deferred-with-owner M11 | M3-C3.3-handoff.md |
| C33-R1-07 | cancellation for an empty render list | divergent  | c33_r1_cancel::empty_frame_observes_requested_cancellation |
| C33-R1-08 | process-death staging cleanup | divergent  | c33_r1_crash::killed_sequence_leaves_no_staged_files |
| REG-render.frame.run | registry row render.frame.run | divergent  | named command suites and reviewer probes |
| REG-render.limits.get | registry row render.limits.get | implemented  | named command suites and reviewer probes |
| REG-render.range.run | registry row render.range.run | divergent  | named command suites and reviewer probes |
| REG-inspect.frame.describe | registry row inspect.frame.describe | divergent  | named command suites and reviewer probes |
| REG-inspect.hitTest | registry row inspect.hitTest | divergent  | named command suites and reviewer probes |
| REG-inspect.coordinates.convert | registry row inspect.coordinates.convert | implemented  | named command suites and reviewer probes |
| REG-inspect.contactSheet.render | registry row inspect.contactSheet.render | divergent  | named command suites and reviewer probes |
| READINESS-FRACTIONAL | fractional-frame coordinate conversion acceptance probe | missing  | c33_r1_contract::fractional_inspection_frame_is_a_typed_registry_refusal |
| READINESS-EVALUATION-COUNT | instrumented count of evaluations in an actual render-list command | missing  | render_list.rs and Evaluator memoization inspected; no public per-query evaluator counter |
| SPEC-WIDTH-DEFAULT | disposition of conflicting generic render width defaults | missing  | default-width.json; default-width.py raw SDK receipt |
