"""C2.3 normalized requirement register; write receipts only with --out."""
import argparse,collections,importlib.util,json,pathlib,tempfile
ROOT=pathlib.Path(__file__).resolve().parents[2]
# Paraphrases identify source requirements; rows are audit units, not a count of independent executions.
# I=implemented, D=divergent, F=deferred with the named owner.
DATA='''02 4.1|Drawing canvas and workspace|F|M4 C4.2
02 4.2|Drawing-view aid and content stack|F|M4 C4.2
02 4.3|Zoom, rotation, pan and alternate view state|F|M4 C4.2
02 4.4|Context and main-menu command bindings|F|M4 C4.2; registry GUI metadata exists
02 4.5|Drawing clipboard structure and colour recovery|I|C2.1/C2.2 predecessor suites, workspace
02 4.5|Delete and selection through the shared command layer|I|c21_draw; c23_select
02 4.5|Point selection routes contour editing|I|c23_transform; c23_select
02 5.1|Tool groups, activation and cycling|F|M4 C4.2
02 5.2|Temporary modifiers, pen-eraser override and gesture constraints|F|M4 C4.2
02 6.1|Interactive pen list, sliders and tool-family memory|F|M4 C4.2
02 6.1|Explicit pen in structural drawing and conversion commands|I|c23_strokeops; c23_cut
02 6.2|Configured nonidentity symmetry and deterministic corpus|F|C21-R1-11; handoff C2.4, traceability still C2.3 (R1-15)
02 6.2|Explicit behind/new-layer placement|I|C2.1 suites; c23_layers
02 6.3|Tablet preference controls|F|M4 C4.2
02 7.1.1|ID, point, rectangle and lasso selections|I|c23_select
02 7.1.1|Selection combination and readable selected points|I|c23_select
02 7.1.1|Translation, scale, rotation and skew matrices|I|c23_transform; c23_r1_contracts
02 7.1.1|Text and bitmap transforms compose their stored matrices|I|c23_transform; ops/transform source
02 7.1.1|Whole affine zone transforms carry fill matrices|I|c23_render upright rotation; c23_transform
02 7.1.1|Copy transforms retain originals and regenerate copied IDs|I|c23_transform
02 7.1.1|Gesture handles, lasso modifiers and pixel nudging|F|M4 C4.2
02 7.1.2|All incident anchors follow a selected shared junction|D|C23-R1-02; c23_r1_commands
02 7.1.2|Contour snap considers segment interiors|D|C23-R1-03; c23_r1_commands
02 7.1.2|Add anchor without changing curve geometry|I|c23_transform
02 7.1.2|Symmetric, broken and reset handle operations|I|c23_transform
02 7.1.2|Point deletion, segment reshaping and direct-outline spine discard|I|c23_transform
02 7.1.2|Smooth supports selected anchor subsets|I|c23_r1_geometry; c23_strokeops
02 7.1.3|Convex projective and nonconvex bilinear envelope geometry|I|c23_transform; vector transform unit tests
02 7.1.3|Envelope keeps painted zones and mapping attachment|D|C23-R1-01; c23_r1_commands
02 7.1.4|Reposition applies the same affine transform across an element|I|c23_transform geometry tests
02 7.1.4|Reposition preserves zone paint and matrices|D|C23-R1-01; c23_r1_commands
02 7.2.1|Brush pens, spines and rigid-motion preservation|I|C2.1 suites; c23_transform
02 7.2.2|Pencil/invisible paths re-node through common settle|I|C2.2 suites; c23_cut
02 7.2.3|Rectangle closure and square/centre constraints|I|c23_select
02 7.2.4|Four-cubic ellipse and circle/centre constraints|I|c23_select
02 7.2.5|Polyline handles and structured closure|I|c23_select; SI-069
02 7.2.5|Interactive continue/add/delete and modifier feedback|F|M4 C4.2
02 7.2.6|Line and constrained/continued endpoints|I|c23_select
02 7.3|Paint, unpaint, gap close and invisible boundaries|I|C2.2 suites retained
02 7.3.7|Structured fill-mapping operations|I|c23_colour; c23_r1_colour
02 7.4.1|Eraser path is geometry, not a file path|I|c21_obligations; c23_cut
02 7.4.1|Eraser line subtraction and zone edges|I|c23_cut
02 7.4.1|Brushes kept whole under the accepted interim cut ruling|I|SI-059/SI-071; c23_cut
02 7.4.2|Polygon, rectangle and segment cuts with fill carry-over|I|c23_cut
02 7.4.2|Wholly enclosed text participates in Cutter selection|D|C23-R1-11; c23_r1_commands
02 7.4.3|Scissor rectangle uses the shared Cutter primitive|I|SI-071; c23_cut
02 7.5|Navigation tools and gestures|F|M4 C4.2
02 7.6|Text creation, edit and run formatting|I|C2.1 text suites
02 7.6|Interactive text editing and panel|F|M4 C4.2
02 7.7|Interactive remembered tool defaults|F|M4 C4.2
02 8.1|Flatten upper/lower precedence and top/bottom position|I|c23_layers; SI-073; policy source inspection
02 8.1|Hidden lower pencil removal and single-layer no-op|I|c23_layers
02 8.1|Flatten work-limit refusal is atomic|I|c23_r1_measure; 0.5360976 seconds
02 8.2|Optimize preserves appearance for thick pencils and upper brushes|D|C23-R1-04; c23_r1_commands
02 8.2|Alpha, gradient and texture overlap stacks stay separate|I|c23_layers
02 8.2|Overlap and point-side decisions use exact ruled grid predicates|D|C23-R1-08; c23_r1_geometry
02 8.3|Smooth tolerance measured in both directions against original|I|c23_r1_geometry dense independent distances
02 8.3|Smooth respects pen/pressure and brush spine authority|I|c23_strokeops
02 8.3|Smooth stays within a charged aggregate work budget|D|C23-R1-07; real release binary 40-second timeout
02 8.4|Arrange preserves stacking order and separates partial layers|I|c23_layers; c23_r1_contracts
02 8.5|Spine-based centre line and no-spine retention|I|c23_strokeops; SI-076
02 8.5|Medial-axis pruning, branches, colour splitting for spineless outlines|F|C23-T1; M2 C2.4
02 8.5|Paint-only selection reports centreLineOnFill|D|C23-R1-12; c23_r1_commands
02 8.6|Invisible centre paths preserve zone closure and fills|I|c23_strokeops
02 8.6|Gradient/texture line conversion succeeds and retains mapping|D|C23-R1-05; c23_r1_commands
02 8.7|Per-character groups use layout_placed positions and source runs|I|c23_contracts; c23_r1_geometry
02 8.7|Glyph counter holes use curve-preserving SI-053 bridges|I|ops/glyphs unit tests; renderer uses nonzero fill
02 8.8|Automatic gap closure and retained invisible segments|I|C2.2 suites and SI-062
02 8.9|Snap preference and gestures|F|M4 C4.2; explicit snap flag is delivered
02 8.10|Draw-on-top matching-colour fusion primitive|I|C2.1 brush suites; SI-073
02 8.11|Draw Behind explicit placement|I|C2.1 drawing suites
02 8.12|Show-strokes render aid and open-end markers|I|c23_render
02 8.13|Fixed rotation and reflection pivots at selection bounds|I|c23_transform
02 8.14|Group/ungroup preserve members and ordering|I|C2.1/C2.2 group suites
02 8.15|Feather bands, step confirmation, texture skips and job cancellation|F|C23-T2; M2 C2.4 after SI-077 spec-owner ruling
02 13.1.1|Commands declare schemas, examples and GUI ownership|I|registry gates; 601 generated declarations current
02 13.1.2|Structural drawing does not require changing tools|I|c21_obligations; source review
02 13.1.4|New shape commands accept element/frame/createDrawing addressing|D|C23-R1-10; c23_r1_commands
02 13.1.5|Scene/view geometry targets and gesture equivalence|F|M3 C3.2 / M4 C4.2
02 13.1.8|Agent operations use typed nonmodal refusals|I|executor gates; bounded exceptions identified separately
02 13.1.9|Seeded IDs and geometry determinism|I|C2.1/C2.2 goldens; c23_render
02 13.1.10|Readable drawing, bounds, selection and virtual view mapping|I|c23_select; SI-069
02 13.1.11|Results and exact changed-ID events for sampled mutations|I|c23_r1_contracts five-command variants
02 13.1.11|Every new mutation has the claimed full common contract|D|C23-R1-14; c23-r1-audit.py
02 13.1.12|Long-work never-block guarantee for Smooth|D|C23-R1-07
02 13.1.13|Batch failure rolls back drawing mutations and render files|I|c23_r1_contracts
02 13.5|Drawing render regions, padding, camera and dimensions|I|c23_render; SI-069/SI-074
02 13.5|Drawing render file output is staged and scope checked|I|c23_r1_contracts; Effect::WriteFile source
02 13.5|Drawing render warnings and pixelsSha256|I|c23_render; SDK envelopes
02 13.5|No machine-font dependency in cross-OS render goldens|I|c23_render fixture source; both CI equality tests
02 13.5|Bitmap-node drawing rendering|F|M3 C3.3; render.bitmapSkipped warning
02 13.6|One drawing event per committed sampled mutation|I|c23_contracts; c23_r1_contracts
03 7.1|Solid alpha compositing|I|render adapter; c23_render gradients/content
03 7.2|Click and drag mapping with the upright texture convention|I|SI-074; c23_render asymmetric 3x2 fixture
03 7.2|Palette first style controls native tile dimensions|I|c22_r3_commands; c22_r4_closure; c23_render
03 7.2|Gradient and texture mappings move with every whole transform|D|C23-R1-01 envelope/reposition exceptions
03 7.2|Split/merge inherits matrices and exact overlap tie-breaking|I|C2.2 rational-overlap suites retained
03 7.3|Linear/radial interpolation and mapped axes|I|c23_render; c23_r1_colour
03 7.4|Tile-on repeat and texture alpha|I|c23_render; render adapter
03 7.4|Tile-off transparency outside mapped image|D|C23-R1-06; c23_r1_texture
03 7.5|Bone-deformable fill behavior|F|M6 C6.3
03 8.1|Painting order, locks and shared transaction layer|I|C2.2 suites; c23_r1_contracts
03 8.2-8.6|Paint/unpaint/dropper/gap closure retained regression surface|I|C2.2 suites; E2 SDK slice
03 8.7|Get matrix and kind-specific handles; typed unmappable results|I|c23_colour; SI-075
03 8.7|Set default/clipboard/matrix/linear/radial/texture mappings|I|c23_colour; c23_r1_colour
03 8.7|Per-object kind skips and session-only clipboard|I|c23_colour; source tool_memory review
03 8.7|Interactive fill handles and context-menu copy/paste|F|M4 C4.2 / M7 C7.1
03 8.8-8.9|Gap-closing persistence and invisible zone boundaries|I|C2.2 suites
03 9.1|Set Colour changes painted selections and leaves unpainted zones|I|c23_colour
03 9.2|Swatch editing, style switching, tints and effects|F|M7 C7.1/C7.2
03 9.3|Drawing/element/scene/project scope and optional pen filters|D|C23-R1-16 scene scope reaches an unexposed bank drawing; c23_r1_colour
03 9.3|Locked skips and changed-reference counts|I|c23_colour
03 9.3|Keep mappable-to-mappable matrices; otherwise default new mapping|I|c23_colour
03 9.3|Protected effective targets with no changes return refusal|D|C23-R1-13; c23_r1_colour
03 9.3|Protection override and mixed protected/free changes|I|c23_colour; SI-061
03 9.4|Draw-top-layer setting UI|F|M4 C4.2
03 16.1.1|Shared command entrypoint and explicit targets|I|SDK run_command and render_drawing
03 16.1.5|Stale zone IDs refuse rather than being silently ignored|D|C23-R1-09 mixed transform target
03 16.1.6|Deterministic repeated render and colour results|I|SDK E2; c23_render
03 16.1.7|Composited dropper pixel in scene/view space|F|M3 C3.2 / M4 C4.2
03 16.1.8|Atomic colour edits, dry-run, undo and persistence|I|c23_contracts; c23_r1_contracts
03 16.3|Remaining palette/style/editor/current/usage commands|F|M7 C7.1
03 16.4|Drawing read-back reflects mapping and recolour commits|I|c23_colour; SDK envelopes'''
def main(out):
    out.mkdir(parents=True,exist_ok=True);states={'I':'implemented','D':'divergent','F':'deferred-with-owner'}
    units=[]
    for line in DATA.splitlines():
        clause,requirement,state,evidence=line.split('|');units.append({'unit':f'C23-U{len(units)+1:03}','clause':clause,'requirement':requirement,'status':states[state],'evidenceOrOwner':evidence})
    spec=importlib.util.spec_from_file_location('audit',pathlib.Path(__file__).with_name('c23-r1-audit.py'));a=importlib.util.module_from_spec(spec);spec.loader.exec_module(a)
    commands=a.inventory()['commands'];divergent={'draw.transform.apply':'02/09','draw.envelope.apply':'01','draw.repositionAll.apply':'01','draw.point.move':'03','draw.cut.apply':'11','draw.optimize.apply':'04/08','draw.smooth.apply':'07','draw.linesToBrush.apply':'05','draw.centreLine.apply':'12; medial axis deferred C23-T1','draw.rectangle.add':'10','draw.ellipse.add':'10','draw.line.add':'10','draw.polyline.add':'10','colour.recolour.apply':'13/16','draw.drawing.render':'06'}
    for c in commands:
        c['status']='divergent' if c['id'] in divergent else 'implemented';c['finding']=divergent.get(c['id']);c['scopeNote']='Semantic disposition; shared addressing and full-contract coverage are audited as separate source units.'
    result={'target':'7a77138c6fb6531b67f4e840b765bd467bb76d28','scope':'Touched requirements of 02 sections 4-8 and 13; 03 sections 7-9 and 16. Normalized source units, not independent test counts. GUI/deferred work has an owner. Implemented means the exercised/inspected slice, not a proof over every input.','counts':dict(collections.Counter(u['status'] for u in units)),'commandCounts':dict(collections.Counter(c['status'] for c in commands)),'units':units,'commands':commands}
    (out/'register.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
    lines=['# C2.3 register','',result['scope'],'',str(result['counts']),'','| Unit | Clause | Requirement | Status | Evidence / owner |','|---|---|---|---|---|']
    lines+=['| '+' | '.join(u[k] for k in ['unit','clause','requirement','status','evidenceOrOwner'])+' |' for u in units]
    (out/'register.md').write_text('\n'.join(lines)+'\n',encoding='utf-8');print(result['counts'],result['commandCounts'])
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--out',type=pathlib.Path);args=p.parse_args()
    if args.out:main(args.out.resolve())
    else:
        with tempfile.TemporaryDirectory(prefix='tt-c23-r1-register-') as tmp:main(pathlib.Path(tmp))
