# Part B option audit

Source comparison: spec/02_drawing.md ??7.6.1, 7.7; drawing.rs default_options/check_option; tool.rs option_schema.

| Tool | Option | Spec default | Spec range | Code default | Code range | Outside refusal |
|---|---|---|---|---|---|---|
| Select | rectRule | inside | inside/intersect | inside | same | tool.valueOutOfRange |
| Pencil, Rectangle, Ellipse, Line, Polyline | thicknessSource; thickness | current pen max | fixed override in ?13.2 | penMax; null | penMax/fixed; null or 0?100 | tool.valueOutOfRange |
| Polyline | snap; snapRadius | true; 8 px | ?13.2: 1?50 | true; 8 | boolean; 1?50 | tool.valueOutOfRange |
| Text | font | platform sans | installed | null | null/nonempty string ?256 bytes | tool.valueOutOfRange |
| Text | size | 36 | 1?512 | 36 | same | tool.valueOutOfRange |
| Text | bold; italic; autoKern | false; false; true | boolean | same | boolean | tool.valueOutOfRange |
| Text | baseline | normal | normal/subscript/superscript | normal | same | tool.valueOutOfRange |
| Text | kerning | 0 | integer ?60?60 | 0 | same | tool.valueOutOfRange |
| Text | align | left | left/centre/right/justify | left | same | tool.valueOutOfRange |
| Text | leftMargin; rightMargin | 0 | 0?1000 | 0 | ?10000?10000 | only beyond code range |
| Text | indent; lineSpacing | 0 | ?99?1000 | 0 | ?10000?10000 | only beyond code range |
| Text | outlineColourId | none | colour ID/none | null | null/nonempty string ?256 | tool.valueOutOfRange |
| Brush; Eraser | penId (family command) | Pen 3; Eraser 2, else first | existing matching family | same | resolved matching family | tool.notApplicable / reference error |
| Contour Editor; Perspective; Reposition All; Paint; Unpaint; Paint Unpainted; Dropper; Close Gap; Stroke; Edit Texture; Cutter; Scissor; Zoom; Grabber | no stored per-tool options | global settings / procedural behavior (?7.7) | ? | empty | unknown option refused | tool.notApplicable |

SI-131 item 5 records the wider text ranges, but ?7.6.1 explicitly supplies ranges. D-10 allows conventions where the spec is silent, so it does not waive these ranges. No failing test asserts installed-font validation, which is not implemented headless.
