# C2.3 register

Touched requirements of 02 sections 4-8 and 13; 03 sections 7-9 and 16. Normalized source units, not independent test counts. GUI/deferred work has an owner. Implemented means the exercised/inspected slice, not a proof over every input.

{'deferred-with-owner': 25, 'implemented': 81, 'divergent': 17}

| Unit | Clause | Requirement | Status | Evidence / owner |
|---|---|---|---|---|
| C23-U001 | 02 4.1 | Drawing canvas and workspace | deferred-with-owner | M4 C4.2 |
| C23-U002 | 02 4.2 | Drawing-view aid and content stack | deferred-with-owner | M4 C4.2 |
| C23-U003 | 02 4.3 | Zoom, rotation, pan and alternate view state | deferred-with-owner | M4 C4.2 |
| C23-U004 | 02 4.4 | Context and main-menu command bindings | deferred-with-owner | M4 C4.2; registry GUI metadata exists |
| C23-U005 | 02 4.5 | Drawing clipboard structure and colour recovery | implemented | C2.1/C2.2 predecessor suites, workspace |
| C23-U006 | 02 4.5 | Delete and selection through the shared command layer | implemented | c21_draw; c23_select |
| C23-U007 | 02 4.5 | Point selection routes contour editing | implemented | c23_transform; c23_select |
| C23-U008 | 02 5.1 | Tool groups, activation and cycling | deferred-with-owner | M4 C4.2 |
| C23-U009 | 02 5.2 | Temporary modifiers, pen-eraser override and gesture constraints | deferred-with-owner | M4 C4.2 |
| C23-U010 | 02 6.1 | Interactive pen list, sliders and tool-family memory | deferred-with-owner | M4 C4.2 |
| C23-U011 | 02 6.1 | Explicit pen in structural drawing and conversion commands | implemented | c23_strokeops; c23_cut |
| C23-U012 | 02 6.2 | Configured nonidentity symmetry and deterministic corpus | deferred-with-owner | C21-R1-11; handoff C2.4, traceability still C2.3 (R1-15) |
| C23-U013 | 02 6.2 | Explicit behind/new-layer placement | implemented | C2.1 suites; c23_layers |
| C23-U014 | 02 6.3 | Tablet preference controls | deferred-with-owner | M4 C4.2 |
| C23-U015 | 02 7.1.1 | ID, point, rectangle and lasso selections | implemented | c23_select |
| C23-U016 | 02 7.1.1 | Selection combination and readable selected points | implemented | c23_select |
| C23-U017 | 02 7.1.1 | Translation, scale, rotation and skew matrices | implemented | c23_transform; c23_r1_contracts |
| C23-U018 | 02 7.1.1 | Text and bitmap transforms compose their stored matrices | implemented | c23_transform; ops/transform source |
| C23-U019 | 02 7.1.1 | Whole affine zone transforms carry fill matrices | implemented | c23_render upright rotation; c23_transform |
| C23-U020 | 02 7.1.1 | Copy transforms retain originals and regenerate copied IDs | implemented | c23_transform |
| C23-U021 | 02 7.1.1 | Gesture handles, lasso modifiers and pixel nudging | deferred-with-owner | M4 C4.2 |
| C23-U022 | 02 7.1.2 | All incident anchors follow a selected shared junction | divergent | C23-R1-02; c23_r1_commands |
| C23-U023 | 02 7.1.2 | Contour snap considers segment interiors | divergent | C23-R1-03; c23_r1_commands |
| C23-U024 | 02 7.1.2 | Add anchor without changing curve geometry | implemented | c23_transform |
| C23-U025 | 02 7.1.2 | Symmetric, broken and reset handle operations | implemented | c23_transform |
| C23-U026 | 02 7.1.2 | Point deletion, segment reshaping and direct-outline spine discard | implemented | c23_transform |
| C23-U027 | 02 7.1.2 | Smooth supports selected anchor subsets | implemented | c23_r1_geometry; c23_strokeops |
| C23-U028 | 02 7.1.3 | Convex projective and nonconvex bilinear envelope geometry | implemented | c23_transform; vector transform unit tests |
| C23-U029 | 02 7.1.3 | Envelope keeps painted zones and mapping attachment | divergent | C23-R1-01; c23_r1_commands |
| C23-U030 | 02 7.1.4 | Reposition applies the same affine transform across an element | implemented | c23_transform geometry tests |
| C23-U031 | 02 7.1.4 | Reposition preserves zone paint and matrices | divergent | C23-R1-01; c23_r1_commands |
| C23-U032 | 02 7.2.1 | Brush pens, spines and rigid-motion preservation | implemented | C2.1 suites; c23_transform |
| C23-U033 | 02 7.2.2 | Pencil/invisible paths re-node through common settle | implemented | C2.2 suites; c23_cut |
| C23-U034 | 02 7.2.3 | Rectangle closure and square/centre constraints | implemented | c23_select |
| C23-U035 | 02 7.2.4 | Four-cubic ellipse and circle/centre constraints | implemented | c23_select |
| C23-U036 | 02 7.2.5 | Polyline handles and structured closure | implemented | c23_select; SI-069 |
| C23-U037 | 02 7.2.5 | Interactive continue/add/delete and modifier feedback | deferred-with-owner | M4 C4.2 |
| C23-U038 | 02 7.2.6 | Line and constrained/continued endpoints | implemented | c23_select |
| C23-U039 | 02 7.3 | Paint, unpaint, gap close and invisible boundaries | implemented | C2.2 suites retained |
| C23-U040 | 02 7.3.7 | Structured fill-mapping operations | implemented | c23_colour; c23_r1_colour |
| C23-U041 | 02 7.4.1 | Eraser path is geometry, not a file path | implemented | c21_obligations; c23_cut |
| C23-U042 | 02 7.4.1 | Eraser line subtraction and zone edges | implemented | c23_cut |
| C23-U043 | 02 7.4.1 | Brushes kept whole under the accepted interim cut ruling | implemented | SI-059/SI-071; c23_cut |
| C23-U044 | 02 7.4.2 | Polygon, rectangle and segment cuts with fill carry-over | implemented | c23_cut |
| C23-U045 | 02 7.4.2 | Wholly enclosed text participates in Cutter selection | divergent | C23-R1-11; c23_r1_commands |
| C23-U046 | 02 7.4.3 | Scissor rectangle uses the shared Cutter primitive | implemented | SI-071; c23_cut |
| C23-U047 | 02 7.5 | Navigation tools and gestures | deferred-with-owner | M4 C4.2 |
| C23-U048 | 02 7.6 | Text creation, edit and run formatting | implemented | C2.1 text suites |
| C23-U049 | 02 7.6 | Interactive text editing and panel | deferred-with-owner | M4 C4.2 |
| C23-U050 | 02 7.7 | Interactive remembered tool defaults | deferred-with-owner | M4 C4.2 |
| C23-U051 | 02 8.1 | Flatten upper/lower precedence and top/bottom position | implemented | c23_layers; SI-073; policy source inspection |
| C23-U052 | 02 8.1 | Hidden lower pencil removal and single-layer no-op | implemented | c23_layers |
| C23-U053 | 02 8.1 | Flatten work-limit refusal is atomic | implemented | c23_r1_measure; 0.5360976 seconds |
| C23-U054 | 02 8.2 | Optimize preserves appearance for thick pencils and upper brushes | divergent | C23-R1-04; c23_r1_commands |
| C23-U055 | 02 8.2 | Alpha, gradient and texture overlap stacks stay separate | implemented | c23_layers |
| C23-U056 | 02 8.2 | Overlap and point-side decisions use exact ruled grid predicates | divergent | C23-R1-08; c23_r1_geometry |
| C23-U057 | 02 8.3 | Smooth tolerance measured in both directions against original | implemented | c23_r1_geometry dense independent distances |
| C23-U058 | 02 8.3 | Smooth respects pen/pressure and brush spine authority | implemented | c23_strokeops |
| C23-U059 | 02 8.3 | Smooth stays within a charged aggregate work budget | divergent | C23-R1-07; real release binary 40-second timeout |
| C23-U060 | 02 8.4 | Arrange preserves stacking order and separates partial layers | implemented | c23_layers; c23_r1_contracts |
| C23-U061 | 02 8.5 | Spine-based centre line and no-spine retention | implemented | c23_strokeops; SI-076 |
| C23-U062 | 02 8.5 | Medial-axis pruning, branches, colour splitting for spineless outlines | deferred-with-owner | C23-T1; M2 C2.4 |
| C23-U063 | 02 8.5 | Paint-only selection reports centreLineOnFill | divergent | C23-R1-12; c23_r1_commands |
| C23-U064 | 02 8.6 | Invisible centre paths preserve zone closure and fills | implemented | c23_strokeops |
| C23-U065 | 02 8.6 | Gradient/texture line conversion succeeds and retains mapping | divergent | C23-R1-05; c23_r1_commands |
| C23-U066 | 02 8.7 | Per-character groups use layout_placed positions and source runs | implemented | c23_contracts; c23_r1_geometry |
| C23-U067 | 02 8.7 | Glyph counter holes use curve-preserving SI-053 bridges | implemented | ops/glyphs unit tests; renderer uses nonzero fill |
| C23-U068 | 02 8.8 | Automatic gap closure and retained invisible segments | implemented | C2.2 suites and SI-062 |
| C23-U069 | 02 8.9 | Snap preference and gestures | deferred-with-owner | M4 C4.2; explicit snap flag is delivered |
| C23-U070 | 02 8.10 | Draw-on-top matching-colour fusion primitive | implemented | C2.1 brush suites; SI-073 |
| C23-U071 | 02 8.11 | Draw Behind explicit placement | implemented | C2.1 drawing suites |
| C23-U072 | 02 8.12 | Show-strokes render aid and open-end markers | implemented | c23_render |
| C23-U073 | 02 8.13 | Fixed rotation and reflection pivots at selection bounds | implemented | c23_transform |
| C23-U074 | 02 8.14 | Group/ungroup preserve members and ordering | implemented | C2.1/C2.2 group suites |
| C23-U075 | 02 8.15 | Feather bands, step confirmation, texture skips and job cancellation | deferred-with-owner | C23-T2; M2 C2.4 after SI-077 spec-owner ruling |
| C23-U076 | 02 13.1.1 | Commands declare schemas, examples and GUI ownership | implemented | registry gates; 601 generated declarations current |
| C23-U077 | 02 13.1.2 | Structural drawing does not require changing tools | implemented | c21_obligations; source review |
| C23-U078 | 02 13.1.4 | New shape commands accept element/frame/createDrawing addressing | divergent | C23-R1-10; c23_r1_commands |
| C23-U079 | 02 13.1.5 | Scene/view geometry targets and gesture equivalence | deferred-with-owner | M3 C3.2 / M4 C4.2 |
| C23-U080 | 02 13.1.8 | Agent operations use typed nonmodal refusals | implemented | executor gates; bounded exceptions identified separately |
| C23-U081 | 02 13.1.9 | Seeded IDs and geometry determinism | implemented | C2.1/C2.2 goldens; c23_render |
| C23-U082 | 02 13.1.10 | Readable drawing, bounds, selection and virtual view mapping | implemented | c23_select; SI-069 |
| C23-U083 | 02 13.1.11 | Results and exact changed-ID events for sampled mutations | implemented | c23_r1_contracts five-command variants |
| C23-U084 | 02 13.1.11 | Every new mutation has the claimed full common contract | divergent | C23-R1-14; c23-r1-audit.py |
| C23-U085 | 02 13.1.12 | Long-work never-block guarantee for Smooth | divergent | C23-R1-07 |
| C23-U086 | 02 13.1.13 | Batch failure rolls back drawing mutations and render files | implemented | c23_r1_contracts |
| C23-U087 | 02 13.5 | Drawing render regions, padding, camera and dimensions | implemented | c23_render; SI-069/SI-074 |
| C23-U088 | 02 13.5 | Drawing render file output is staged and scope checked | implemented | c23_r1_contracts; Effect::WriteFile source |
| C23-U089 | 02 13.5 | Drawing render warnings and pixelsSha256 | implemented | c23_render; SDK envelopes |
| C23-U090 | 02 13.5 | No machine-font dependency in cross-OS render goldens | implemented | c23_render fixture source; both CI equality tests |
| C23-U091 | 02 13.5 | Bitmap-node drawing rendering | deferred-with-owner | M3 C3.3; render.bitmapSkipped warning |
| C23-U092 | 02 13.6 | One drawing event per committed sampled mutation | implemented | c23_contracts; c23_r1_contracts |
| C23-U093 | 03 7.1 | Solid alpha compositing | implemented | render adapter; c23_render gradients/content |
| C23-U094 | 03 7.2 | Click and drag mapping with the upright texture convention | implemented | SI-074; c23_render asymmetric 3x2 fixture |
| C23-U095 | 03 7.2 | Palette first style controls native tile dimensions | implemented | c22_r3_commands; c22_r4_closure; c23_render |
| C23-U096 | 03 7.2 | Gradient and texture mappings move with every whole transform | divergent | C23-R1-01 envelope/reposition exceptions |
| C23-U097 | 03 7.2 | Split/merge inherits matrices and exact overlap tie-breaking | implemented | C2.2 rational-overlap suites retained |
| C23-U098 | 03 7.3 | Linear/radial interpolation and mapped axes | implemented | c23_render; c23_r1_colour |
| C23-U099 | 03 7.4 | Tile-on repeat and texture alpha | implemented | c23_render; render adapter |
| C23-U100 | 03 7.4 | Tile-off transparency outside mapped image | divergent | C23-R1-06; c23_r1_texture |
| C23-U101 | 03 7.5 | Bone-deformable fill behavior | deferred-with-owner | M6 C6.3 |
| C23-U102 | 03 8.1 | Painting order, locks and shared transaction layer | implemented | C2.2 suites; c23_r1_contracts |
| C23-U103 | 03 8.2-8.6 | Paint/unpaint/dropper/gap closure retained regression surface | implemented | C2.2 suites; E2 SDK slice |
| C23-U104 | 03 8.7 | Get matrix and kind-specific handles; typed unmappable results | implemented | c23_colour; SI-075 |
| C23-U105 | 03 8.7 | Set default/clipboard/matrix/linear/radial/texture mappings | implemented | c23_colour; c23_r1_colour |
| C23-U106 | 03 8.7 | Per-object kind skips and session-only clipboard | implemented | c23_colour; source tool_memory review |
| C23-U107 | 03 8.7 | Interactive fill handles and context-menu copy/paste | deferred-with-owner | M4 C4.2 / M7 C7.1 |
| C23-U108 | 03 8.8-8.9 | Gap-closing persistence and invisible zone boundaries | implemented | C2.2 suites |
| C23-U109 | 03 9.1 | Set Colour changes painted selections and leaves unpainted zones | implemented | c23_colour |
| C23-U110 | 03 9.2 | Swatch editing, style switching, tints and effects | deferred-with-owner | M7 C7.1/C7.2 |
| C23-U111 | 03 9.3 | Drawing/element/scene/project scope and optional pen filters | implemented | c23_colour; source scope traversal review |
| C23-U112 | 03 9.3 | Locked skips and changed-reference counts | implemented | c23_colour |
| C23-U113 | 03 9.3 | Keep mappable-to-mappable matrices; otherwise default new mapping | implemented | c23_colour |
| C23-U114 | 03 9.3 | Protected effective targets with no changes return refusal | divergent | C23-R1-13; c23_r1_colour |
| C23-U115 | 03 9.3 | Protection override and mixed protected/free changes | implemented | c23_colour; SI-061 |
| C23-U116 | 03 9.4 | Draw-top-layer setting UI | deferred-with-owner | M4 C4.2 |
| C23-U117 | 03 16.1.1 | Shared command entrypoint and explicit targets | implemented | SDK run_command and render_drawing |
| C23-U118 | 03 16.1.5 | Stale zone IDs refuse rather than being silently ignored | divergent | C23-R1-09 mixed transform target |
| C23-U119 | 03 16.1.6 | Deterministic repeated render and colour results | implemented | SDK E2; c23_render |
| C23-U120 | 03 16.1.7 | Composited dropper pixel in scene/view space | deferred-with-owner | M3 C3.2 / M4 C4.2 |
| C23-U121 | 03 16.1.8 | Atomic colour edits, dry-run, undo and persistence | implemented | c23_contracts; c23_r1_contracts |
| C23-U122 | 03 16.3 | Remaining palette/style/editor/current/usage commands | deferred-with-owner | M7 C7.1 |
| C23-U123 | 03 16.4 | Drawing read-back reflects mapping and recolour commits | implemented | c23_colour; SDK envelopes |
