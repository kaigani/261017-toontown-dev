# Fixtures of the M4 manual scripts

| File | Used by | Contents |
|---|---|---|
| `c43-camera.animpkg` | `C4.3-camera-view.md`, `C4.3-scene-tools.md`, `C4.3-coexistence.md` | Bob (`el_01K7JJN800FWYC1TEZ57XR65SW`), a black rectangle at the centre; Ann (`el_01K7JJN80055VQSF7WREM7JZEP`), a black ellipse to the left and behind him; frames 1-24 |

Copy a fixture somewhere writable before opening it. `c43-camera.animpkg` is rebuilt, from commands only, by
`python scripts/make-c43-manual-fixture.py <toontown-headless binary>`; the element ids above are those of the committed
file; a rebuild allocates new ids (the scripts name the committed ones), so use the committed file.
