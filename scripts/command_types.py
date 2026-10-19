"""Type catalogue for scripts/gen-command-declarations.py (C12-R2-02).

Collects the result and parameter shapes the owning sections define (spec 01-09, read-only) and turns them into closed
JSON Schemas. Three spellings occur in the sections:

- `Name = { ... }` inside a fenced block (07 §15.4);
- a bold heading just above a ```json block: `**NodeEval** (...)` (05 §19.4);
- `// Name` markers inside a ```jsonc block (08 §19.3, 09 §11.4), with `{ /* Name */ }` references to other types,
  `"a|b|c"` enumerations, enumerations in comments (`"none" /* none|mean|gaussian */`), optional members marked by a
  comment (`/* only if requested */`), and `// Name = Base + { ... }` extensions.

Schemas inferred from examples are closed (`additionalProperties: false` at every object level); a member is required
unless a comment marks it optional; leaf types come from the example values, refined by the field-name rules of
`leaf_for_name` (frames and counts are integers, `...Id` fields are IDs, flags are booleans). Placeholders in the
examples (`[16 numbers]`, ellipses) are read as what they stand for.
"""
import json
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]
SPEC = ROOT / "spec"

ID = {"type": "string", "minLength": 1, "maxLength": 4096, "description": "object ID or path reference (10 §3)"}
PATH = {"type": "string", "minLength": 1, "maxLength": 4096, "description": "file-system path (11 §6.2)"}
NUMBER = {"type": "number"}
INTEGER = {"type": "integer"}
BOOLEAN = {"type": "boolean"}
STRING = {"type": "string", "maxLength": 65536}
COLOUR = {"type": "string", "pattern": "^#[0-9A-Fa-f]{6}([0-9A-Fa-f]{2})?$", "description": "#RRGGBB or #RRGGBBAA"}
# A leaf whose type no section states: a scalar (never an object or a list), recorded as a spec issue.
SCALAR_PLACEHOLDER = {"type": ["string", "number", "boolean", "null"], "x-placeholder": True,
                      "description": "type not stated by the owning section (spec issue); a scalar"}

INT_NAMES = re.compile(
    r"^(frame|frames|startFrame|endFrame|stopFrame|naturalEndFrame|localFrame|globalFrame|globalStart|globalEnd|"
    r"globalLength|from|to|start|end|count|index|depth|rows|columns|copies|loops|rev|documentRev|documentRevision|"
    r"width|height|length|step|size|limit|offset|page|volume|quality|seed|level|minHoldFrames|"
    r"anticipationFrames|bucketsPerFrame|frameCount|frameOffset|linkOffset|sourceLength|framesWritten|"
    r"framesRendered|framesSimulated|framesDone|framesTotal|captures|completed|editPoint|sceneCount|maxSize|"
    r"jpegQuality|interval|intensity|bytes|sampleRate|droppedFrames|elapsedMs|nextFrame|paddingFrames|"
    r"durationLeft|durationRight|blankDuration|columnWidth|maxPreviewFps|pagesPrinted|elementCount|"
    r"drawingCount|usageCount|position)$"
    r"|.*(Count|Frames|Index|Bytes|Width|Height|Frame|Ms)$")
NUMBER_NAMES = re.compile(
    r"^(x|y|z|w|h|dx|dy|dz|dt|dv|minX|minY|maxX|maxY|time|value|zoom|scale|rotation|angle|skew|opacity|progress|"
    r"tension|continuity|bias|velocity|fov|near|far|min|max|rms|peakDb|rmsDb|inPoint|outPoint|"
    r"arcLength|ratio|threshold|tolerance|strength|amount|gain|speed|rate|transparentFraction|percent|fps|"
    r"measuredFps|orientAngle|estimatedBytes|etaSec|frameRate)$|.*(Sec|Seconds|Db|Ratio|Scale|Angle|Time|Percent|Mm)$")
BOOL_NAMES = re.compile(
    r"^(enabled|visible|playing|missing|stale|connected|pending|remembered|writable|closed|deleted|cancelled|"
    r"locked|dirty|active|solo|collapsed|modified|readOnly|timedOut|alpha|clamped|applied|selected|checked|"
    r"suppressed|isDefault|manual|silent|hasLipSync|lipSyncStale|elementExtended|dryRun|lockDepth|"
    r"constantSegment|keyframeCreated|available|valid|ok|current|focused|open|exists|warning|empty|feeder|async|"
    r"overwrite|recursive|inherited|builtIn|loop|forceRate|sound|scrubbing|running|singleDrawing|protected|"
    r"supported|previewPaused|matte|hideBackground|advance|noiseReduction|lumaCorrection|boundingKeyframes|"
    r"overwriteDrawings|createExposure|extendExposures|updateColours|keyframeSubstitution|playing)$"
    r"|^(is|has|can|should|was|needs)[A-Z].*")
ID_NAMES = re.compile(r"^(id|.*Id)$")
IDS_NAMES = re.compile(r"^(ids|.*Ids)$")
COLOUR_NAMES = re.compile(r"^(colour|keyColour|backgroundColour|markerColour|.*Colour)$")
STRING_NAMES = re.compile(
    r"^(name|label|title|message|code|description|type|kind|mode|format|url|language|status|text|source|channel|"
    r"phoneme|mouth|drawing|location|assetName|context|keys|preset|curveType|handleMode|commandId|command|"
    r"service|account|accountName|version|state|reason|unit|style|font|tag|summary|data|thumbnail|preview|"
    r"origin|createdAt|modifiedAt|created|modified|startTime|endTime|libraryName|role|codec|family|phase|"
    r"setting|direction|reference|aspectRatio|mimeType|sha256|png|storedAs|file|range|resolution|filter|"
    r"artist|copyright|versionId|formatVersion|fileName|mediaType|key)$|.*(Name|Url|Code|Mode|Kind|Type)$")
PATH_NAMES = re.compile(r"^(path|outputPath|pdfOutputPath|folderPath|.*Path)$")


POINT_NAMES = re.compile(r"^(point|centre|center|anchor|pivot|worldPosition|offset2d|origin2d|hotspot)$")
RECT_NAMES = re.compile(r"^(bounds|printableArea|visibleRect|rect|bbox|area)$")
EXTRA_NUMBERS = re.compile(
    r"^(rotationDegrees|duration|t|p|pressure|minSize|smoothness|roll|effectiveFov|zoomLevel|age|freedMB|"
    r"cacheUsedMB|cacheBudgetMB|systemMemoryMB|recommendedTextureCacheMB|effectiveFps|depthNudgeStep|"
    r"defaultTension|defaultContinuity|defaultBias|captureSpeed|total|lightTableShade|movieOpacity)$")
EXTRA_INTEGERS = re.compile(r"^(sceneLength|cycleLength|bitDepth|fileSize|maxInlinePixels|maxTextureSize|rowsSelected)$")
EXTRA_BOOLEANS = re.compile(
    r"^(placeBehind|drawTopLayer|snapToContour|symmetry|approximated|reusedAsset|decodable|complete|optional|"
    r"fitToCamera|preserveImageLayers|preserveVectorLayers|dark|hidden|deformable|descendantKeyed|constant|"
    r"boxHighlighting|keepSelectionVisible|linearByDefault|constantKeysByDefault|autoExtendExposure|"
    r"gpuAccelerated|interactiveAntialiasingSupported|restartRequired|exceedsLimit|durationChanged|committed|"
    r"deviceAvailable|autoReapply|showAdvanced|renamed|saved|required|duplex|waveforms|allPegs|pegGhosts|arms|"
    r"frameMarkers|envelopes|antialiasToolOverlays|currentOnTop|autoLightTable|staticLightTable|onionSkin|grid|"
    r"overlay|animated|conflict|agentIndicator|tablet|mirrors|strokes|zones|lines|brushStrokes|effects|"
    r"selectionDisplay|playback|changed)$")
EXTRA_STRINGS = re.compile(
    r"^(onConflict|tool|tipShape|variant|category|operation|orientation|paperSize|audioFormat|displayUnits|"
    r"outputDevice|view|projection|extension|option|gapClosing|onionSkinStyle|defaultRange|exposureText|"
    r"preset|background|aid|keyedChannel|param|rendering|visibility|actual|change|target|slot|group|swatch|placement|"
    r"before|after|previous|next|small|medium|large|linear|radial|texture|resolved|enabledRange|conflictMode)$")
EXTRA_ID_LISTS = re.compile(r"^(removed|added|updated|created|movedKeys|mergedKeys|skipped|elementList|notes)$")


def leaf_for_name(name: str):
    """A leaf schema from a field name alone, or None when the name says nothing."""
    if name in ("left", "right"):
        # A key's handle (05 §9): offsets in frames and value units.
        return {"type": "object", "properties": {"dt": dict(NUMBER), "dv": dict(NUMBER)}, "required": ["dt", "dv"],
                "additionalProperties": False}
    if POINT_NAMES.match(name):
        return {"type": "object", "properties": {"x": dict(NUMBER), "y": dict(NUMBER)}, "required": ["x", "y"],
                "additionalProperties": False}
    if RECT_NAMES.match(name):
        return {"type": "object", "properties": {k: dict(NUMBER) for k in ("x", "y", "width", "height")},
                "required": ["x", "y", "width", "height"], "additionalProperties": False}
    if EXTRA_NUMBERS.match(name):
        return dict(NUMBER)
    if EXTRA_INTEGERS.match(name):
        return dict(INTEGER)
    if EXTRA_BOOLEANS.match(name):
        return dict(BOOLEAN)
    if EXTRA_STRINGS.match(name):
        return dict(STRING)
    if EXTRA_ID_LISTS.match(name):
        return {"type": "array", "items": dict(ID), "maxItems": 100000}
    if name == "lastFolder":
        return dict(PATH)
    if PATH_NAMES.match(name):
        return dict(PATH)
    if IDS_NAMES.match(name):
        return {"type": "array", "items": dict(ID), "maxItems": 100000}
    if ID_NAMES.match(name):
        return dict(ID)
    if COLOUR_NAMES.match(name):
        return dict(COLOUR)
    if BOOL_NAMES.match(name):
        return dict(BOOLEAN)
    if INT_NAMES.match(name):
        return dict(INTEGER)
    if NUMBER_NAMES.match(name):
        return dict(NUMBER)
    if STRING_NAMES.match(name):
        return dict(STRING)
    return None


ENUM_TEXT = re.compile(r"[A-Za-z0-9_]+(\|[A-Za-z0-9_]+)+")
OPTIONAL_COMMENT = re.compile(
    r'"(\w+)"\s*:[^,{}\[\]]*?(?:\{[^{}]*\})?\s*/\*[^*]*\b(?:only|optional|or|if|absent)\b[^*]*\*/')


def _clean_json(text: str):
    """Spec example text as JSON, and the members its comments mark optional."""
    t = re.sub(r"\[\s*\d+\s+numbers\s*\]", "[0.5]", text)
    t = re.sub(r"\{\s*/\*\s*([A-Z][A-Za-z]+)\s*\*/\s*\}", r'{"$typeRef": "\1"}', t)
    t = re.sub(r"\[\s*/\*\s*([A-Z][A-Za-z]+)\s*\*/\s*\]", r'[{"$typeRef": "\1"}]', t)
    t = re.sub(r'"[A-Za-z0-9_]+"\s*/\*\s*([A-Za-z0-9_]+(?:\|[A-Za-z0-9_]+)+)\s*\*/', r'"\1"', t)
    optional = set(OPTIONAL_COMMENT.findall(t))
    t = re.sub(r"/\*.*?\*/", "", t, flags=re.S)
    t = re.sub(r"//[^\n]*", "", t)
    t = t.replace("…", "").replace("...", "")
    t = re.sub(r",\s*,", ",", t)
    t = re.sub(r",\s*([}\]])", r"\1", t)
    return t, optional


def _parse(text: str):
    cleaned, optional = _clean_json(text)
    try:
        return json.loads(cleaned), optional
    except ValueError:
        return None, set()


def _type_of(v):
    if v is None:
        return "null"
    if isinstance(v, bool):
        return "boolean"
    if isinstance(v, int):
        return "integer"
    if isinstance(v, float):
        return "number"
    if isinstance(v, str):
        return "string"
    if isinstance(v, list):
        return "array"
    return "object"


def _singular(name: str) -> str:
    if name.endswith("ies"):
        return name[:-3] + "y"
    return name[:-1] if name.endswith("s") and not name.endswith("ss") else name


def infer(values: list, name: str = "", optional: frozenset = frozenset()):
    """A closed schema for every value in `values` (examples of one position)."""
    kinds = {_type_of(v) for v in values}
    nullable = "null" in kinds
    kinds.discard("null")
    named = leaf_for_name(name)
    values = [v for v in values if v is not None]
    if not kinds:
        if named is None:
            return dict(SCALAR_PLACEHOLDER)
        return {"anyOf": [named, {"type": "null"}]}
    if kinds <= {"integer", "number"}:
        if named and named.get("type") in ("integer", "number"):
            base = named
        else:
            base = dict(NUMBER)
    elif kinds == {"string"}:
        if all(ENUM_TEXT.fullmatch(v) for v in values):
            enum = [x for v in values for x in v.split("|")]
            base = {"type": "string", "enum": sorted(set(enum), key=enum.index)}
        elif named and named.get("type") == "string":
            base = named
        else:
            base = dict(STRING)
    elif kinds == {"boolean"}:
        base = dict(BOOLEAN)
    elif kinds == {"array"}:
        items = [x for v in values for x in v]
        if items:
            item = infer(items, _singular(name), optional)
        else:
            item = leaf_for_name(_singular(name)) or dict(SCALAR_PLACEHOLDER)
        base = {"type": "array", "items": item, "maxItems": 100000}
    elif kinds == {"object"} and all(set(v) == {"$typeRef"} for v in values):
        base = {"$ref": f"#/$defs/{values[0]['$typeRef']}"}
    elif kinds == {"object"}:
        objs = [v for v in values if isinstance(v, dict)]
        keys = []
        for o in objs:
            for k in o:
                if k not in keys:
                    keys.append(k)
        props = {k: infer([o[k] for o in objs if k in o], k, optional) for k in keys}
        required = [k for k in keys if all(k in o for o in objs) and k not in optional]
        base = {"type": "object", "properties": props, "additionalProperties": False}
        if required:
            base["required"] = required
    else:
        alts = []
        for k in sorted(kinds):
            same = [v for v in values if _type_of(v) == k]
            alts.append(infer(same, name, optional))
        base = {"anyOf": alts}
    if nullable:
        return {"anyOf": [base, {"type": "null"}]}
    return base


def _blocks(text: str):
    """(preceding text, body) of every fenced block."""
    return [(text[max(0, m.start() - 600):m.start()], m.group(1))
            for m in re.finditer(r"```[a-zA-Z]*\n(.*?)```", text, re.S)]


def _add(types: dict, unparsed: list, section: str, name: str, body: str, base=None):
    v, optional = _parse(body)
    if v is None:
        unparsed.append(f"{section}:{name}")
        return
    schema = infer([v], name, frozenset(optional))
    if base:
        schema = {"allOf": [{"$ref": f"#/$defs/{base}"}], "x-extends": base, **schema}
    schema["x-specSection"] = section
    types.setdefault(name, schema)


def catalogue():
    """{TypeName: schema} from every owning section, and the names whose examples did not parse."""
    types: dict = {}
    unparsed: list = []
    for f in sorted(SPEC.glob("0*.md")):
        text = f.read_text(encoding="utf-8")
        section = f.name.split("_")[0]
        for before, body in _blocks(text):
            marked = list(re.finditer(r"^//\s*([A-Z][A-Za-z]+)(?:\s*=\s*([A-Z][A-Za-z]+)\s*\+)?[^\n]*$", body, re.M))
            if marked:
                for i, m in enumerate(marked):
                    end = marked[i + 1].start() if i + 1 < len(marked) else len(body)
                    _add(types, unparsed, section, m.group(1), body[m.end():end], m.group(2))
                continue
            named = list(re.finditer(r"^([A-Z][A-Za-z]+)\s*=\s*(?=[{\[])", body, re.M))
            if named:
                for i, m in enumerate(named):
                    end = named[i + 1].start() if i + 1 < len(named) else len(body)
                    _add(types, unparsed, section, m.group(1), body[m.end():end])
                continue
            heads = re.findall(r"\*\*`?([A-Z][A-Za-z]+)`?\*\*[^\n]*\n\s*$", before)
            if heads:
                _add(types, unparsed, section, heads[-1], body)
    return types, unparsed


if __name__ == "__main__":
    t, bad = catalogue()
    print(len(t), sorted(t))
    print("unparsed:", bad)
