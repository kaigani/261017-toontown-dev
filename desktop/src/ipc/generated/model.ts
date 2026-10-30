// GENERATED FILE: do not edit.
// Source: Rust types via schemars (crates/engine/src/wire.rs, crates/model/src/schema.rs).
// Regenerate: TT_UPDATE_GOLDEN=1 cargo test -p tt-engine --test frontend_schemas
import type { Schema } from '../schema';

export const MODEL_FORMAT_VERSION = "1.0";

export const OBJECT_TYPE_DEFS = {
  project: "Project",
  palette: "Palette",
  paletteStyle: "PaletteStyle",
  swatch: "Swatch",
  pen: "Pen",
  asset: "Asset",
  drawingBank: "DrawingBank",
  drawing: "Drawing",
  drawingNode: "DrawingNode",
  stroke: "Stroke",
  scene: "Scene",
  transition: "Transition",
  element: "Element",
  channel: "Channel",
  key: "Key",
  motionPoint: "MotionPoint",
  particleBlock: "ParticleBlock",
  soundPlacement: "SoundPlacement",
  mediaSequence: "MediaSequence",
  library: "Library",
  libraryFolder: "LibraryFolder",
  libraryItem: "LibraryItem",
};

export const MODEL_DEFS: Readonly<Record<string, Schema>> = {
  Alignment: {
    oneOf: [
      {
        type: "string",
        const: "left",
        description: "Left.",
      },
      {
        type: "string",
        const: "centre",
        description: "Centre.",
      },
      {
        type: "string",
        const: "right",
        description: "Right.",
      },
      {
        type: "string",
        const: "justify",
        description: "Justify.",
      },
    ],
    description: "Paragraph alignment.",
  },
  ApplyPivotTo: {
    oneOf: [
      {
        type: "string",
        const: "rotation",
        description: "Rotation only (default).",
      },
      {
        type: "string",
        const: "scaling",
        description: "Scaling only.",
      },
      {
        type: "string",
        const: "both",
        description: "Both.",
      },
    ],
    description: "Pivot application of pegs and skeletons (05 §2.3).",
  },
  Asset: {
    type: "object",
    additionalProperties: false,
    properties: {
      id: {
        $ref: "#/$defs/Id",
        description: "Immutable ID.",
        readOnly: true,
        "x-since": "1.0",
      },
      type: {
        type: "string",
        const: "asset",
        readOnly: true,
        description: "Always `asset`.",
        "x-since": "1.0",
      },
      rev: {
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
        description: "Revision of the last change.",
        readOnly: true,
        "x-since": "1.0",
      },
      name: {
        pattern: "^(?:[^\\u0000-\\u001F\\u007F\\s]|[^\\u0000-\\u001F\\u007F\\s][^\\u0000-\\u001F\\u007F]*[^\\u0000-\\u001F\\u007F\\s])$",
        description: "Informational name (file name by default).",
        minLength: 1,
        maxLength: 255,
        type: "string",
        "x-since": "1.0",
      },
      kind: {
        $ref: "#/$defs/AssetKind",
        description: "Media kind.",
        "x-since": "1.0",
      },
      mediaType: {
        type: "string",
        description: "IANA media type, e.g. `audio/wav`.",
        minLength: 1,
        maxLength: 255,
        "x-since": "1.0",
      },
      storage: {
        $ref: "#/$defs/AssetStorage",
        description: "Embedded or linked.",
        "x-since": "1.0",
      },
      entry: {
        type: [
          "string",
          "null",
        ],
        description: "Package entry path when embedded (`media/<assetId>.<ext>`); `null` when linked.",
        "x-since": "1.0",
      },
      link: {
        anyOf: [
          {
            $ref: "#/$defs/AssetLink",
          },
          {
            type: "null",
          },
        ],
        description: "The link when linked; `null` when embedded.",
        "x-since": "1.0",
      },
      size: {
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
        description: "Size of the file bytes (64-bit).",
        "x-unit": "byte",
        "x-since": "1.0",
      },
      sha256: {
        type: "string",
        description: "Lower-case hex SHA-256 of the file bytes.",
        pattern: "^[0-9a-f]{64}$",
        "x-since": "1.0",
      },
      importedAt: {
        $ref: "#/$defs/Timestamp",
        description: "Import time.",
        readOnly: true,
        "x-since": "1.0",
      },
      sourcePath: {
        type: [
          "string",
          "null",
        ],
        description: "Where the file was imported from (informational).",
        "x-since": "1.0",
      },
      meta: {
        type: "object",
        additionalProperties: true,
        description: "Free-form metadata.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      ext: {
        $ref: "#/$defs/Ext",
        description: "Extension data.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      path: {
        type: "string",
        description: "Canonical path (10 §2.4).",
        "x-transient": true,
        readOnly: true,
        "x-since": "1.0",
      },
      info: {
        type: "object",
        "x-derived": true,
        readOnly: true,
        "x-since": "1.0",
      },
      refs: {
        type: "array",
        items: {
          $ref: "#/$defs/Id",
        },
        "x-derived": true,
        readOnly: true,
        "x-since": "1.0",
      },
      status: {
        enum: [
          "ok",
          "missing",
          "changed",
          "unsupported",
        ],
        "x-transient": true,
        readOnly: true,
        "x-since": "1.0",
      },
    },
    "x-since": "1.0",
    description: "The project-wide registry entry of one media file (10 §4.16).",
    "x-objectType": "asset",
    required: [
      "id",
      "type",
    ],
    if: {
      required: [
        "rev",
      ],
    },
    then: {
      required: [
        "id",
        "type",
        "rev",
        "name",
        "kind",
        "mediaType",
        "storage",
        "entry",
        "link",
        "size",
        "sha256",
        "importedAt",
        "sourcePath",
      ],
    },
    else: {
      propertyNames: {
        enum: [
          "id",
          "type",
          "name",
        ],
      },
    },
  },
  AssetKind: {
    oneOf: [
      {
        type: "string",
        const: "sound",
        description: "Audio.",
      },
      {
        type: "string",
        const: "image",
        description: "Bitmap image.",
      },
      {
        type: "string",
        const: "movie",
        description: "Movie.",
      },
      {
        type: "string",
        const: "vector",
        description: "Vector file linked by media elements.",
      },
    ],
    description: "Media kind of an asset.",
  },
  AssetLink: {
    type: "object",
    additionalProperties: false,
    properties: {
      kind: {
        $ref: "#/$defs/LinkKind",
        description: "File or library item.",
        "x-since": "1.0",
      },
      path: {
        type: [
          "string",
          "null",
        ],
        description: "Files: path relative to the folder containing the project.",
        "x-since": "1.0",
      },
      absolutePath: {
        type: [
          "string",
          "null",
        ],
        description: "Files: absolute path at link time.",
        "x-since": "1.0",
      },
      libraryId: {
        anyOf: [
          {
            $ref: "#/$defs/Id",
          },
          {
            type: "null",
          },
        ],
        description: "Library items: library ID.",
        "x-since": "1.0",
      },
      itemId: {
        anyOf: [
          {
            $ref: "#/$defs/Id",
          },
          {
            type: "null",
          },
        ],
        description: "Library items: item ID.",
        "x-since": "1.0",
      },
      versionId: {
        type: [
          "string",
          "null",
        ],
        description: "Library items: template version seen at link time (08 LIB-007).",
        "x-since": "1.0",
      },
    },
    required: [
      "kind",
      "path",
      "absolutePath",
      "libraryId",
      "itemId",
      "versionId",
    ],
    description: "The link of a `linked` asset (10 §4.16). Fields that do not apply to the link kind are `null`.",
  },
  AssetStorage: {
    oneOf: [
      {
        type: "string",
        const: "embedded",
        description: "Copied into the package under `media/`.",
      },
      {
        type: "string",
        const: "linked",
        description: "Referenced in place (file or library item).",
      },
    ],
    description: "Where the bytes live (10 §6.7).",
  },
  AxesTransform: {
    type: "object",
    additionalProperties: false,
    properties: {
      xAxis: {
        $ref: "#/$defs/Point2",
        description: "Image of the x axis.",
        "x-since": "1.0",
      },
      yAxis: {
        $ref: "#/$defs/Point2",
        description: "Image of the y axis.",
        "x-since": "1.0",
      },
      origin: {
        $ref: "#/$defs/Point2",
        description: "Origin (fields).",
        "x-unit": "field",
        "x-since": "1.0",
      },
    },
    required: [
      "xAxis",
      "yAxis",
      "origin",
    ],
    description: "A 2-D affine transform as axes and origin (06 §3.4 shadow transform).",
  },
  Blob: {
    type: "object",
    additionalProperties: false,
    properties: {
      entry: {
        type: "string",
        description: "Package entry path.",
        minLength: 1,
        maxLength: 1024,
        "x-since": "1.0",
      },
      mediaType: {
        type: "string",
        description: "IANA media type.",
        minLength: 1,
        maxLength: 255,
        "x-since": "1.0",
      },
      size: {
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
        description: "Size in bytes (64-bit: entries larger than 4 GiB are supported, 10 §5).",
        "x-since": "1.0",
      },
      sha256: {
        type: "string",
        description: "Lower-case hex SHA-256 of the bytes.",
        pattern: "^[0-9a-f]{64}$",
        "x-since": "1.0",
      },
    },
    required: [
      "entry",
      "mediaType",
      "size",
      "sha256",
    ],
    description: "A binary payload stored as a package entry (10 §1.1). Never inlined in object JSON.",
  },
  BlockStatic: {
    type: "object",
    additionalProperties: false,
    properties: {
      background: {
        $ref: "#/$defs/ParticleBackground",
        description: "Background drawn behind the particles.",
        "x-since": "1.0",
      },
      simulationStart: {
        type: "integer",
        maximum: 2147483647,
        description: "First simulated frame (≤ firstFrame, ≥ firstFrame − 999; may be < 1 for pre-roll).",
        "x-unit": "frame",
        minimum: -2147483648,
        "x-since": "1.0",
      },
      firstFrame: {
        type: "integer",
        maximum: 2147483647,
        minimum: 1,
        description: "First displayed frame.",
        "x-unit": "frame",
        "x-since": "1.0",
      },
      duration: {
        type: "integer",
        maximum: 2147483647,
        minimum: 1,
        description: "Frames the block lasts, ≥ 1.",
        "x-unit": "frame",
        "x-since": "1.0",
      },
      maxParticles: {
        type: "integer",
        maximum: 3000,
        minimum: 0,
        description: "Particle cap (≤ 3000; 50 for fog and cloud).",
        "x-since": "1.0",
      },
      lifetimeFrames: {
        type: "integer",
        "x-unit": "frame",
        description: "Lifetime in frames: −1 (unlimited) or 0–512.",
        minimum: -1,
        maximum: 512,
        "x-since": "1.0",
      },
      fade: {
        type: "boolean",
        description: "Fade particles out.",
        "x-since": "1.0",
      },
      areaWidth: {
        type: "number",
        maximum: 1999,
        description: "Region width 0–1999 particle units.",
        minimum: 0,
        "x-since": "1.0",
      },
      areaHeight: {
        type: "number",
        maximum: 1999,
        description: "Region height 0–1999.",
        minimum: 0,
        "x-since": "1.0",
      },
      areaDepth: {
        type: "number",
        maximum: 1999,
        description: "Region depth 0–1999.",
        minimum: 0,
        "x-since": "1.0",
      },
      floorLevel: {
        type: "number",
        maximum: 1999,
        description: "Floor height 0–1999.",
        minimum: 0,
        "x-since": "1.0",
      },
    },
    required: [
      "background",
      "simulationStart",
      "firstFrame",
      "duration",
      "maxParticles",
      "lifetimeFrames",
      "fade",
      "areaWidth",
      "areaHeight",
      "areaDepth",
      "floorLevel",
    ],
    description: "Static parameters of a particle block (06 §3.8.3). Frames are absolute scene frames.",
  },
  BlurParams: {
    type: "object",
    additionalProperties: false,
    properties: {
      radial: {
        type: "boolean",
        description: "Uniform (true) or directional (false) blur (static).",
        "x-since": "1.0",
      },
      radius: {
        type: "number",
        "x-unit": "pixel",
        description: "Radius 0–100 output pixels (animatable).",
        minimum: 0,
        maximum: 100,
        "x-since": "1.0",
      },
      angle: {
        type: "number",
        "x-unit": "degree",
        description: "Motion-blur direction 0–360 degrees (animatable).",
        minimum: 0,
        maximum: 360,
        "x-since": "1.0",
      },
    },
    required: [
      "radial",
      "radius",
      "angle",
    ],
    description: "Blur parameters (06 §3.5).",
  },
  BoneBlock: {
    type: "object",
    additionalProperties: false,
    properties: {
      rest: {
        $ref: "#/$defs/BoneRest",
        description: "Rest pose.",
        "x-since": "1.0",
      },
      influence: {
        $ref: "#/$defs/BoneInfluence",
        description: "Zone of influence.",
        "x-since": "1.0",
      },
    },
    required: [
      "rest",
      "influence",
    ],
    description: "Bone block (10 §4.6, 05 §13.1–§13.3).",
  },
  BoneInfluence: {
    type: "object",
    additionalProperties: false,
    properties: {
      type: {
        $ref: "#/$defs/InfluenceType",
        description: "Infinite or elliptical.",
        "x-since": "1.0",
      },
      lengthRadius: {
        type: "number",
        "x-unit": "field",
        description: "Length radius (fields).",
        minimum: 0,
        "x-since": "1.0",
      },
      widthRadius: {
        type: "number",
        "x-unit": "field",
        description: "Width radius (fields).",
        minimum: 0,
        "x-since": "1.0",
      },
      region: {
        type: [
          "array",
          "null",
        ],
        items: {
          $ref: "#/$defs/Point2",
        },
        description: "Explicit region polygon, or `null`.",
        "x-since": "1.0",
      },
    },
    required: [
      "type",
      "lengthRadius",
      "widthRadius",
      "region",
    ],
    description: "Zone of influence of a bone (05 §13.3).",
  },
  BoneRest: {
    type: "object",
    additionalProperties: false,
    properties: {
      x: {
        type: "number",
        "x-unit": "field",
        description: "X (fields).",
        "x-since": "1.0",
      },
      y: {
        type: "number",
        "x-unit": "field",
        description: "Y (fields).",
        "x-since": "1.0",
      },
      rotation: {
        type: "number",
        "x-unit": "degree",
        description: "Rotation (degrees).",
        "x-since": "1.0",
      },
      length: {
        type: "number",
        "x-unit": "field",
        description: "Length (fields), > 0.01.",
        minimum: 0.01,
        exclusiveMinimum: 0.01,
        "x-since": "1.0",
      },
    },
    required: [
      "x",
      "y",
      "rotation",
      "length",
    ],
    description: "Rest pose of a bone (05 §13.1).",
  },
  CameraBlock: {
    type: "object",
    additionalProperties: false,
    properties: {
      fovMode: {
        $ref: "#/$defs/FovMode",
        description: "Static or dynamic FOV.",
        "x-command": "anim.camera.set",
        "x-since": "1.0",
      },
      fov: {
        type: "number",
        "x-command": "anim.camera.set",
        description: "FOV 1–179 degrees (default 41.1121).",
        minimum: 1,
        maximum: 179,
        "x-unit": "degree",
        "x-since": "1.0",
      },
    },
    required: [
      "fovMode",
      "fov",
    ],
    description: "Camera block (10 §4.6, 05 §12.3).",
  },
  Channel: {
    type: "object",
    additionalProperties: false,
    properties: {
      id: {
        $ref: "#/$defs/Id",
        description: "Immutable ID.",
        readOnly: true,
        "x-since": "1.0",
      },
      type: {
        type: "string",
        const: "channel",
        readOnly: true,
        description: "Always `channel`.",
        "x-since": "1.0",
      },
      rev: {
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
        description: "Revision of the last change.",
        readOnly: true,
        "x-since": "1.0",
      },
      name: {
        pattern: "^[a-z][A-Za-z]*(\\.[a-z][A-Za-z]*)?$",
        description: "The channel id.",
        readOnly: true,
        minLength: 1,
        maxLength: 64,
        type: "string",
        "x-since": "1.0",
      },
      curveType: {
        "x-kindSpecific": true,
        description: "1-D channels: curve type.",
        $ref: "#/$defs/CurveType",
        "x-since": "1.0",
      },
      keys: {
        type: "array",
        items: {
          $ref: "#/$defs/Key",
        },
        description: "1-D channels: keys sorted by frame, at least one.",
        "x-kindSpecific": true,
        "x-since": "1.0",
      },
      points: {
        type: "array",
        items: {
          $ref: "#/$defs/MotionPoint",
        },
        description: "Motion path: points in path order; first and last are keyframes.",
        "x-kindSpecific": true,
        "x-since": "1.0",
      },
      meta: {
        type: "object",
        additionalProperties: true,
        description: "Free-form metadata.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      ext: {
        $ref: "#/$defs/Ext",
        description: "Extension data.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      path: {
        type: "string",
        description: "Canonical path (10 §2.4).",
        "x-transient": true,
        readOnly: true,
        "x-since": "1.0",
      },
    },
    "x-since": "1.0",
    description: "An animation channel (10 §4.12). The `name` is the channel id (`rotation`, `scale.x`, …, or an effect\nparameter id) and equals its key in the element's `channels` map. A 1-D channel has `curveType` and `keys`;\nthe motion-path channel `position` has `points` instead.",
    "x-objectType": "channel",
    required: [
      "id",
      "type",
    ],
    if: {
      required: [
        "rev",
      ],
    },
    then: {
      required: [
        "id",
        "type",
        "rev",
        "name",
      ],
      allOf: [
        {
          if: {
            properties: {
              name: {
                const: "position",
              },
            },
            required: [
              "name",
            ],
          },
          then: {
            required: [
              "points",
            ],
            properties: {
              curveType: false,
              keys: false,
              points: {
                minItems: 1,
              },
            },
          },
          else: {
            required: [
              "curveType",
              "keys",
            ],
            properties: {
              points: false,
              keys: {
                minItems: 1,
              },
            },
          },
        },
      ],
    },
    else: {
      propertyNames: {
        enum: [
          "id",
          "type",
          "name",
        ],
      },
    },
  },
  ClippingParams: {
    type: "object",
    additionalProperties: false,
    properties: {
      inverted: {
        type: "boolean",
        description: "Draw children outside the mask instead of inside.",
        "x-since": "1.0",
      },
    },
    required: [
      "inverted",
    ],
    description: "Clipping parameters (06 §3.3).",
  },
  Colour: {
    type: "string",
    pattern: "^#([0-9A-Fa-f]{6}|[0-9A-Fa-f]{8})$",
    description: "sRGB colour with straight alpha, #RRGGBBAA (written upper case; #RRGGBB accepted on input).",
  },
  ColourRecoveryEntry: {
    type: "object",
    additionalProperties: false,
    properties: {
      colourId: {
        $ref: "#/$defs/Id",
        description: "The colour ID.",
        "x-since": "1.0",
      },
      paletteId: {
        $ref: "#/$defs/Id",
        description: "Owning palette ID.",
        "x-since": "1.0",
      },
      paletteName: {
        type: "string",
        description: "Owning palette name.",
        "x-since": "1.0",
      },
      styles: {
        type: "array",
        items: {
          $ref: "#/$defs/RecoveryStyle",
        },
        description: "The palette's styles.",
        "x-since": "1.0",
      },
      swatch: {
        $ref: "#/$defs/Swatch",
        description: "The full swatch (all style values).",
        "x-since": "1.0",
      },
    },
    required: [
      "colourId",
      "paletteId",
      "paletteName",
      "styles",
      "swatch",
    ],
    description: "A recovery copy of one swatch used by a drawing (10 §4.11, 03 §11.2).",
  },
  ColourTransformParams: {
    type: "object",
    additionalProperties: false,
    properties: {
      multRed: {
        type: "number",
        maximum: 1,
        description: "Red multiplier 0–1 (animatable).",
        minimum: 0,
        "x-since": "1.0",
      },
      multGreen: {
        type: "number",
        maximum: 1,
        description: "Green multiplier 0–1 (animatable).",
        minimum: 0,
        "x-since": "1.0",
      },
      multBlue: {
        type: "number",
        maximum: 1,
        description: "Blue multiplier 0–1 (animatable).",
        minimum: 0,
        "x-since": "1.0",
      },
      multAlpha: {
        type: "number",
        maximum: 1,
        description: "Alpha multiplier 0–1 (animatable).",
        minimum: 0,
        "x-since": "1.0",
      },
      addRed: {
        type: "integer",
        maximum: 255,
        description: "Red offset −255…255 (animatable).",
        minimum: -255,
        "x-since": "1.0",
      },
      addGreen: {
        type: "integer",
        maximum: 255,
        description: "Green offset −255…255 (animatable).",
        minimum: -255,
        "x-since": "1.0",
      },
      addBlue: {
        type: "integer",
        maximum: 255,
        description: "Blue offset −255…255 (animatable).",
        minimum: -255,
        "x-since": "1.0",
      },
      addAlpha: {
        type: "integer",
        maximum: 255,
        description: "Alpha offset −255…255 (animatable).",
        minimum: -255,
        "x-since": "1.0",
      },
      flatten: {
        type: "boolean",
        description: "Composite the sub-tree first (static).",
        "x-since": "1.0",
      },
    },
    required: [
      "multRed",
      "multGreen",
      "multBlue",
      "multAlpha",
      "addRed",
      "addGreen",
      "addBlue",
      "addAlpha",
      "flatten",
    ],
    description: "Colour Transform parameters (06 §3.2).",
  },
  CurveType: {
    oneOf: [
      {
        type: "string",
        const: "spline",
        description: "Cubic Bézier with free handles.",
      },
      {
        type: "string",
        const: "interpolation",
        description: "Bézier with an easing preset per segment.",
      },
      {
        type: "string",
        const: "velocityBased",
        description: "Progress follows the velocity channel.",
      },
    ],
    description: "Curve type of a 1-D channel (05 §10.3).",
  },
  Drawing: {
    type: "object",
    additionalProperties: false,
    properties: {
      id: {
        $ref: "#/$defs/Id",
        description: "Immutable ID.",
        readOnly: true,
        "x-since": "1.0",
      },
      type: {
        type: "string",
        const: "drawing",
        readOnly: true,
        description: "Always `drawing`.",
        "x-since": "1.0",
      },
      rev: {
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
        description: "Revision of the last change (the drawing revision of 02 §3.12).",
        readOnly: true,
        "x-since": "1.0",
      },
      name: {
        pattern: "^(?:[^\\u0000-\\u001F\\u007F\\\\/:*?\"<>|.\\s]|[^\\u0000-\\u001F\\u007F\\\\/:*?\"<>|.\\s][^\\u0000-\\u001F\\u007F\\\\/:*?\"<>|.]*[^\\u0000-\\u001F\\u007F\\\\/:*?\"<>|.\\s])$",
        description: "Unique in the bank; default next unused integer.",
        "x-command": "element.drawing.rename",
        minLength: 1,
        maxLength: 64,
        type: "string",
        "x-since": "1.0",
      },
      locked: {
        type: "boolean",
        description: "Locked drawings reject content edits with `drawing.locked`.",
        "x-command": "element.drawingLock.set",
        "x-since": "1.0",
      },
      pivot: {
        $ref: "#/$defs/Point2",
        description: "Drawing pivot (du).",
        "x-unit": "du",
        "x-since": "1.0",
      },
      mouthShape: {
        anyOf: [
          {
            $ref: "#/$defs/MouthShape",
          },
          {
            type: "null",
          },
        ],
        description: "Mouth shape for automatic lip-sync mapping.",
        "x-since": "1.0",
      },
      nodes: {
        type: "array",
        items: {
          $ref: "#/$defs/DrawingNode",
        },
        description: "Nodes, back to front.",
        "x-since": "1.0",
      },
      colourRecovery: {
        type: "array",
        items: {
          $ref: "#/$defs/ColourRecoveryEntry",
        },
        description: "One entry per colour ID the drawing uses (written on save).",
        "x-since": "1.0",
      },
      createdAt: {
        $ref: "#/$defs/Timestamp",
        description: "Creation time.",
        readOnly: true,
        "x-since": "1.0",
      },
      meta: {
        type: "object",
        additionalProperties: true,
        description: "Free-form metadata.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      ext: {
        $ref: "#/$defs/Ext",
        description: "Extension data.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      path: {
        type: "string",
        description: "Canonical path (10 §2.4).",
        "x-transient": true,
        readOnly: true,
        "x-since": "1.0",
      },
      thumbnail: {
        anyOf: [
          {
            $ref: "#/$defs/Blob",
          },
          {
            type: "null",
          },
        ],
        "x-derived": true,
        readOnly: true,
        "x-since": "1.0",
      },
      usage: {
        type: "array",
        items: {
          type: "object",
          properties: {
            elementId: {
              $ref: "#/$defs/Id",
              "x-since": "1.0",
            },
            ranges: {
              type: "array",
              items: {
                $ref: "#/$defs/FrameRange",
              },
              "x-since": "1.0",
            },
          },
          required: [
            "elementId",
            "ranges",
          ],
          additionalProperties: false,
        },
        "x-derived": true,
        readOnly: true,
        "x-since": "1.0",
      },
      isEmpty: {
        type: "boolean",
        "x-derived": true,
        readOnly: true,
        "x-since": "1.0",
      },
      zones: {
        type: "array",
        items: {
          type: "object",
        },
        "x-derived": true,
        readOnly: true,
        "x-since": "1.0",
      },
    },
    "x-since": "1.0",
    description: "A drawing (10 §4.8). Stored in its own package entry `banks/<bankId>/<drawingId>.json`.",
    "x-objectType": "drawing",
    required: [
      "id",
      "type",
    ],
    if: {
      required: [
        "rev",
      ],
    },
    then: {
      required: [
        "id",
        "type",
        "rev",
        "name",
        "locked",
        "pivot",
        "mouthShape",
        "nodes",
        "colourRecovery",
        "createdAt",
      ],
    },
    else: {
      propertyNames: {
        enum: [
          "id",
          "type",
          "name",
        ],
      },
    },
  },
  DrawingBank: {
    type: "object",
    additionalProperties: false,
    properties: {
      id: {
        $ref: "#/$defs/Id",
        description: "Immutable ID.",
        readOnly: true,
        "x-since": "1.0",
      },
      type: {
        type: "string",
        const: "drawingBank",
        readOnly: true,
        description: "Always `drawingBank`.",
        "x-since": "1.0",
      },
      rev: {
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
        description: "Revision of the last change.",
        readOnly: true,
        "x-since": "1.0",
      },
      drawings: {
        type: "array",
        items: {
          $ref: "#/$defs/Drawing",
        },
        "x-childList": "drawing",
        description: "Drawings (unordered; written in natural name order).",
        "x-since": "1.0",
      },
      meta: {
        type: "object",
        additionalProperties: true,
        description: "Free-form metadata.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      ext: {
        $ref: "#/$defs/Ext",
        description: "Extension data.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      path: {
        type: "string",
        description: "Canonical path (10 §2.4).",
        "x-transient": true,
        readOnly: true,
        "x-since": "1.0",
      },
      elementIds: {
        type: "array",
        items: {
          $ref: "#/$defs/Id",
        },
        "x-derived": true,
        readOnly: true,
        "x-since": "1.0",
      },
    },
    "x-since": "1.0",
    description: "The drawings of one clone group (10 §4.8). Stored in `banks/<bankId>/bank.json` with drawing stubs.",
    "x-objectType": "drawingBank",
    required: [
      "id",
      "type",
    ],
    if: {
      required: [
        "rev",
      ],
    },
    then: {
      required: [
        "id",
        "type",
        "rev",
        "drawings",
      ],
    },
    else: {
      propertyNames: {
        enum: [
          "id",
          "type",
          "name",
        ],
      },
    },
  },
  DrawingNode: {
    type: "object",
    additionalProperties: false,
    properties: {
      id: {
        $ref: "#/$defs/Id",
        description: "ID, unique within the drawing (ID-03).",
        readOnly: true,
        "x-since": "1.0",
      },
      type: {
        type: "string",
        const: "drawingNode",
        readOnly: true,
        description: "Always `drawingNode`.",
        "x-since": "1.0",
      },
      rev: {
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
        description: "Revision of the last change.",
        readOnly: true,
        "x-since": "1.0",
      },
      name: {
        pattern: "^(?:[^\\u0000-\\u001F\\u007F\\s]|[^\\u0000-\\u001F\\u007F\\s][^\\u0000-\\u001F\\u007F]*[^\\u0000-\\u001F\\u007F\\s])$",
        description: "Optional name.",
        "x-omitDefault": true,
        minLength: 1,
        maxLength: 255,
        type: "string",
        "x-since": "1.0",
      },
      kind: {
        $ref: "#/$defs/NodeKind",
        description: "Layer, group, text or bitmap; read-only.",
        readOnly: true,
        "x-since": "1.0",
      },
      hint: {
        "x-kindSpecific": true,
        description: "Layer: type hint.",
        $ref: "#/$defs/LayerHint",
        "x-since": "1.0",
      },
      strokes: {
        type: "array",
        items: {
          $ref: "#/$defs/Stroke",
        },
        description: "Layer: strokes.",
        "x-kindSpecific": true,
        "x-since": "1.0",
      },
      children: {
        type: "array",
        items: {
          $ref: "#/$defs/DrawingNode",
        },
        description: "Group: child nodes, back to front.",
        "x-kindSpecific": true,
        "x-since": "1.0",
      },
      transform: {
        type: "array",
        items: {
          type: "number",
        },
        minItems: 6,
        maxItems: 6,
        description: "Text and bitmap: placement in drawing space.",
        "x-kindSpecific": true,
        "x-since": "1.0",
      },
      widthMode: {
        "x-kindSpecific": true,
        description: "Text: auto or fixed width.",
        $ref: "#/$defs/WidthMode",
        "x-since": "1.0",
      },
      width: {
        type: "number",
        "x-kindSpecific": true,
        description: "Text: wrap width in du (fixed mode); bitmap: pixel width 1–16384.",
        "x-since": "1.0",
      },
      height: {
        type: "number",
        "x-kindSpecific": true,
        description: "Bitmap: pixel height 1–16384.",
        "x-since": "1.0",
      },
      text: {
        type: "string",
        description: "Text: the characters (≤ 100 000).",
        maxLength: 100000,
        "x-kindSpecific": true,
        "x-since": "1.0",
      },
      runs: {
        type: "array",
        items: {
          $ref: "#/$defs/TextRun",
        },
        description: "Text: character runs covering the text.",
        "x-kindSpecific": true,
        "x-since": "1.0",
      },
      paragraphs: {
        type: "array",
        items: {
          $ref: "#/$defs/Paragraph",
        },
        description: "Text: paragraphs.",
        "x-kindSpecific": true,
        "x-since": "1.0",
      },
      image: {
        "x-kindSpecific": true,
        description: "Bitmap: PNG pixels (8-bit RGBA, straight alpha).",
        $ref: "#/$defs/Blob",
        "x-since": "1.0",
      },
      meta: {
        type: "object",
        additionalProperties: true,
        description: "Free-form metadata.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      ext: {
        $ref: "#/$defs/Ext",
        description: "Extension data.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      path: {
        type: "string",
        description: "Canonical path (10 §2.4).",
        "x-transient": true,
        readOnly: true,
        "x-since": "1.0",
      },
    },
    "x-since": "1.0",
    description: "A node of a drawing (10 §4.9): a layer (`hint`, `strokes`), a group (`children`), a text object (`transform`,\n`widthMode`, `width`, `text`, `runs`, `paragraphs`) or a bitmap (`image`, `width`, `height`, `transform`).\nProperties of other kinds are absent.",
    "x-objectType": "drawingNode",
    required: [
      "id",
      "type",
    ],
    if: {
      required: [
        "rev",
      ],
    },
    then: {
      required: [
        "id",
        "type",
        "rev",
        "kind",
      ],
      allOf: [
        {
          if: {
            properties: {
              kind: {
                const: "layer",
              },
            },
            required: [
              "kind",
            ],
          },
          then: {
            required: [
              "hint",
              "strokes",
            ],
            properties: {
              children: false,
              transform: false,
              widthMode: false,
              width: false,
              height: false,
              text: false,
              runs: false,
              paragraphs: false,
              image: false,
            },
          },
        },
        {
          if: {
            properties: {
              kind: {
                const: "group",
              },
            },
            required: [
              "kind",
            ],
          },
          then: {
            required: [
              "children",
            ],
            properties: {
              hint: false,
              strokes: false,
              transform: false,
              widthMode: false,
              width: false,
              height: false,
              text: false,
              runs: false,
              paragraphs: false,
              image: false,
            },
          },
        },
        {
          if: {
            properties: {
              kind: {
                const: "text",
              },
            },
            required: [
              "kind",
            ],
          },
          then: {
            required: [
              "transform",
              "widthMode",
              "text",
              "runs",
              "paragraphs",
            ],
            properties: {
              hint: false,
              strokes: false,
              children: false,
              height: false,
              image: false,
              width: {
                type: "number",
                minimum: 0,
                "x-unit": "du",
              },
            },
          },
        },
        {
          if: {
            properties: {
              kind: {
                const: "bitmap",
              },
            },
            required: [
              "kind",
            ],
          },
          then: {
            required: [
              "image",
              "width",
              "height",
              "transform",
            ],
            properties: {
              hint: false,
              strokes: false,
              children: false,
              widthMode: false,
              text: false,
              runs: false,
              paragraphs: false,
              width: {
                type: "integer",
                minimum: 1,
                maximum: 16384,
              },
              height: {
                type: "integer",
                minimum: 1,
                maximum: 16384,
              },
            },
          },
        },
      ],
    },
    else: {
      propertyNames: {
        enum: [
          "id",
          "type",
          "name",
        ],
      },
    },
  },
  DropShadowParams: {
    type: "object",
    additionalProperties: false,
    properties: {
      parentColourTransformMode: {
        $ref: "#/$defs/ParentColourTransformMode",
        description: "How a parent Colour Transform applies.",
        "x-since": "1.0",
      },
      colour: {
        $ref: "#/$defs/Colour",
        description: "Shadow colour (alpha ignored).",
        "x-since": "1.0",
      },
      opacity: {
        type: "integer",
        "x-unit": "percent",
        minimum: 0,
        description: "Opacity percent 0–100.",
        maximum: 100,
        "x-since": "1.0",
      },
      hideElements: {
        type: "boolean",
        description: "Draw only the shadow.",
        "x-since": "1.0",
      },
      transform: {
        $ref: "#/$defs/AxesTransform",
        description: "Shadow transform in effect-local space.",
        "x-since": "1.0",
      },
    },
    required: [
      "parentColourTransformMode",
      "colour",
      "opacity",
      "hideElements",
      "transform",
    ],
    description: "Drop Shadow parameters (06 §3.4; all static).",
  },
  DynamicParam: {
    anyOf: [
      {
        type: "object",
        properties: {
          value: {
            type: "number",
            description: "The value.",
            "x-since": "1.0",
          },
        },
        required: [
          "value",
        ],
        description: "Constant value.",
      },
      {
        type: "object",
        properties: {
          keyframes: {
            type: "array",
            items: {
              $ref: "#/$defs/FrameValue",
            },
            description: "Keyframes sorted by frame, at least one.",
            minItems: 1,
            "x-since": "1.0",
          },
        },
        required: [
          "keyframes",
        ],
        description: "Keyframed value.",
      },
    ],
    description: "A dynamic block parameter: a constant `{value}` or `{keyframes}` (06 §8.4).",
  },
  Easing: {
    oneOf: [
      {
        type: "string",
        const: "constant",
        description: "Hold, then step.",
      },
      {
        type: "string",
        const: "linear",
        description: "Straight line.",
      },
      {
        type: "string",
        const: "easeInOut",
        description: "Slow start, slow end.",
      },
      {
        type: "string",
        const: "fastInOut",
        description: "Fast start, fast end.",
      },
      {
        type: "string",
        const: "extremeEaseInOut",
        description: "Strong slow start and end.",
      },
      {
        type: "string",
        const: "extremeFastInOut",
        description: "Strong fast start and end.",
      },
      {
        type: "string",
        const: "slowAcceleration",
        description: "Ease in.",
      },
      {
        type: "string",
        const: "slowDeceleration",
        description: "Ease out.",
      },
    ],
    description: "Easing presets (05 §10.4).",
  },
  Element: {
    type: "object",
    additionalProperties: false,
    properties: {
      id: {
        $ref: "#/$defs/Id",
        description: "Immutable ID.",
        readOnly: true,
        "x-since": "1.0",
      },
      type: {
        type: "string",
        const: "element",
        readOnly: true,
        description: "Always `element`.",
        "x-since": "1.0",
      },
      rev: {
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
        description: "Revision of the last change.",
        readOnly: true,
        "x-since": "1.0",
      },
      name: {
        pattern: "^(?:[^\\u0000-\\u001F\\u007F\\\\/:*?\"<>|.\\s]|[^\\u0000-\\u001F\\u007F\\\\/:*?\"<>|.\\s][^\\u0000-\\u001F\\u007F\\\\/:*?\"<>|.]*[^\\u0000-\\u001F\\u007F\\\\/:*?\"<>|.\\s])$",
        description: "Unique in the scene.",
        "x-command": "element.rename",
        minLength: 1,
        maxLength: 64,
        type: "string",
        "x-since": "1.0",
      },
      elementType: {
        $ref: "#/$defs/ElementType",
        description: "Element type; read-only except through conversion commands.",
        readOnly: true,
        "x-since": "1.0",
      },
      start: {
        type: "integer",
        maximum: 2147483647,
        minimum: 1,
        description: "Non-cell elements: first frame of the bar (persisted). Cell-based elements: derived from the exposure (query results only).",
        "x-unit": "frame",
        "x-kindSpecific": true,
        "x-since": "1.0",
      },
      duration: {
        type: "integer",
        maximum: 2147483647,
        minimum: 0,
        description: "Non-cell elements: length of the bar (absent for cell-based elements: derived).",
        "x-unit": "frame",
        "x-kindSpecific": true,
        "x-since": "1.0",
      },
      loops: {
        type: "integer",
        maximum: 2147483647,
        minimum: 1,
        description: "Loop count ≥ 1; 1 = not looped.",
        "x-since": "1.0",
      },
      loopType: {
        $ref: "#/$defs/LoopType",
        description: "Count or fill.",
        "x-since": "1.0",
      },
      positionLoop: {
        $ref: "#/$defs/PositionLoop",
        description: "Cycle or offset.",
        "x-since": "1.0",
      },
      role: {
        $ref: "#/$defs/Role",
        description: "Composite role.",
        "x-since": "1.0",
      },
      deformable: {
        type: "boolean",
        description: "Deformable by a skeleton.",
        "x-since": "1.0",
      },
      visibleInScene: {
        type: "boolean",
        description: "Shown in scene views, playback and export.",
        "x-command": "element.visibility.set",
        "x-since": "1.0",
      },
      visibleInXsheet: {
        type: "boolean",
        description: "Cell-based elements: shown in the Exposure Sheet and Drawing View.",
        "x-command": "element.visibility.set",
        "x-kindSpecific": true,
        "x-since": "1.0",
      },
      locked: {
        type: "boolean",
        description: "Locked in scene views.",
        "x-command": "element.lock.set",
        "x-since": "1.0",
      },
      collapsed: {
        type: "boolean",
        description: "Timeline display state of parents.",
        "x-since": "1.0",
      },
      markerColour: {
        anyOf: [
          {
            $ref: "#/$defs/Colour",
          },
          {
            type: "null",
          },
        ],
        description: "Marker colour; `null` = category default.",
        "x-command": "element.colour.set",
        "x-since": "1.0",
      },
      backgroundColour: {
        anyOf: [
          {
            $ref: "#/$defs/Colour",
          },
          {
            type: "null",
          },
        ],
        description: "Background colour; `null` = category default.",
        "x-command": "element.colour.set",
        "x-since": "1.0",
      },
      note: {
        type: "string",
        description: "Element note (≤ 10 000 characters).",
        maxLength: 10000,
        "x-omitDefault": true,
        "x-command": "element.note.set",
        "x-since": "1.0",
      },
      columnWidth: {
        type: "integer",
        "x-unit": "pixel",
        minimum: 20,
        description: "Exposure Sheet column width, 20–1000 view pixels.",
        maximum: 1000,
        "x-since": "1.0",
      },
      drawingBankId: {
        "x-kindSpecific": true,
        description: "Cell-based: the drawing bank (shared by clones).",
        "x-ref": "drawingBank",
        readOnly: true,
        $ref: "#/$defs/Id",
        "x-since": "1.0",
      },
      exposure: {
        type: "array",
        items: {
          type: "array",
          prefixItems: [
            {
              anyOf: [
                {
                  $ref: "#/$defs/Id",
                },
                {
                  type: "null",
                },
              ],
            },
            {
              type: "integer",
              maximum: 2147483647,
              minimum: 1,
            },
          ],
          minItems: 2,
          maxItems: 2,
        },
        description: "Cell-based: canonical exposure runs from frame 1.",
        "x-kindSpecific": true,
        "x-since": "1.0",
      },
      cellNotes: {
        type: "object",
        additionalProperties: false,
        patternProperties: {
          "^(?:[1-9][0-9]{0,8}|1[0-9]{9}|20[0-9]{8}|21[0-3][0-9]{7}|214[0-6][0-9]{6}|2147[0-3][0-9]{5}|21474[0-7][0-9]{4}|214748[0-2][0-9]{3}|2147483[0-5][0-9]{2}|21474836[0-3][0-9]|214748364[0-7])$": {
            type: "string",
            maxLength: 10000,
          },
        },
        description: "Cell-based: notes by frame.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      importOptions: {
        type: "object",
        additionalProperties: true,
        description: "Image elements: import options (section 09; SI-008).",
        "x-kindSpecific": true,
        "x-since": "1.0",
      },
      streamed: {
        type: "boolean",
        description: "Sound: streamed playback.",
        "x-kindSpecific": true,
        "x-since": "1.0",
      },
      showMouthShapes: {
        type: "boolean",
        description: "Sound: the Exposure Sheet shows mouths.",
        "x-kindSpecific": true,
        "x-since": "1.0",
      },
      placements: {
        type: "array",
        items: {
          $ref: "#/$defs/SoundPlacement",
        },
        description: "Sound: placements sorted by `startFrame`, non-overlapping.",
        "x-kindSpecific": true,
        "x-since": "1.0",
      },
      lipSyncMappings: {
        type: "array",
        items: {
          $ref: "#/$defs/LipSyncMapping",
        },
        description: "Sound: one mapping per mapped drawing element.",
        "x-kindSpecific": true,
        "x-since": "1.0",
      },
      sequences: {
        type: "array",
        items: {
          $ref: "#/$defs/MediaSequence",
        },
        description: "Media: sequences, non-overlapping.",
        "x-kindSpecific": true,
        "x-since": "1.0",
      },
      transform: {
        "x-kindSpecific": true,
        description: "Transformable elements: the static transform.",
        $ref: "#/$defs/Transform",
        "x-since": "1.0",
      },
      exportGroup: {
        "x-kindSpecific": true,
        description: "Pegs: export grouping.",
        $ref: "#/$defs/ExportGroup",
        "x-since": "1.0",
      },
      camera: {
        "x-kindSpecific": true,
        description: "Cameras: FOV settings.",
        $ref: "#/$defs/CameraBlock",
        "x-since": "1.0",
      },
      skeleton: {
        "x-kindSpecific": true,
        description: "Skeletons: display and deformation settings.",
        $ref: "#/$defs/SkeletonBlock",
        "x-since": "1.0",
      },
      bone: {
        "x-kindSpecific": true,
        description: "Bones: rest pose and influence.",
        $ref: "#/$defs/BoneBlock",
        "x-since": "1.0",
      },
      slot: {
        readOnly: true,
        description: "Effect parameter elements: the slot.",
        "x-kindSpecific": true,
        $ref: "#/$defs/ParamSlot",
        "x-since": "1.0",
      },
      params: {
        type: "object",
        additionalProperties: true,
        description: "Effects: static parameter values (exact shape per `elementType`).",
        "x-kindSpecific": true,
        "x-since": "1.0",
      },
      blocks: {
        type: "array",
        items: {
          $ref: "#/$defs/ParticleBlock",
        },
        description: "Particle effects: blocks, consecutive.",
        "x-kindSpecific": true,
        "x-since": "1.0",
      },
      channels: {
        type: "object",
        additionalProperties: {
          $ref: "#/$defs/Channel",
        },
        description: "Animated channels by channel id.",
        "x-kindSpecific": true,
        "x-since": "1.0",
      },
      createdAt: {
        $ref: "#/$defs/Timestamp",
        description: "Creation time.",
        readOnly: true,
        "x-since": "1.0",
      },
      meta: {
        type: "object",
        additionalProperties: true,
        description: "Free-form metadata.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      ext: {
        $ref: "#/$defs/Ext",
        description: "Extension data.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      path: {
        type: "string",
        description: "Canonical path (10 §2.4).",
        "x-transient": true,
        readOnly: true,
        "x-since": "1.0",
      },
      parentId: {
        anyOf: [
          {
            $ref: "#/$defs/Id",
          },
          {
            type: "null",
          },
        ],
        "x-derived": true,
        readOnly: true,
        "x-since": "1.0",
      },
      childIds: {
        type: "array",
        items: {
          $ref: "#/$defs/Id",
        },
        "x-derived": true,
        readOnly: true,
        "x-since": "1.0",
      },
      depth: {
        type: "integer",
        minimum: 0,
        maximum: 2147483647,
        "x-derived": true,
        readOnly: true,
        "x-since": "1.0",
      },
      timelineIndex: {
        type: "integer",
        minimum: 0,
        maximum: 2147483647,
        "x-derived": true,
        readOnly: true,
        "x-since": "1.0",
      },
      xsheetIndex: {
        anyOf: [
          {
            type: "integer",
            minimum: 0,
            maximum: 2147483647,
          },
          {
            type: "null",
          },
        ],
        "x-derived": true,
        readOnly: true,
        "x-since": "1.0",
      },
      cloneGroupId: {
        anyOf: [
          {
            $ref: "#/$defs/Id",
          },
          {
            type: "null",
          },
        ],
        "x-derived": true,
        readOnly: true,
        "x-since": "1.0",
      },
      cloneIds: {
        type: "array",
        items: {
          $ref: "#/$defs/Id",
        },
        "x-derived": true,
        readOnly: true,
        "x-since": "1.0",
      },
      drawingCount: {
        type: "integer",
        minimum: 0,
        maximum: 2147483647,
        "x-derived": true,
        readOnly: true,
        "x-since": "1.0",
      },
      hiddenByParent: {
        type: "boolean",
        "x-derived": true,
        readOnly: true,
        "x-since": "1.0",
      },
      effectiveMarkerColour: {
        $ref: "#/$defs/Colour",
        "x-derived": true,
        readOnly: true,
        "x-since": "1.0",
      },
    },
    "x-since": "1.0",
    description: "An element of a scene (10 §4.5–§4.7). Stored in `scenes/<sceneId>/elements/<elementId>.json`.",
    "x-objectType": "element",
    required: [
      "id",
      "type",
    ],
    if: {
      required: [
        "rev",
      ],
    },
    then: {
      required: [
        "id",
        "type",
        "rev",
        "name",
        "elementType",
        "loops",
        "loopType",
        "positionLoop",
        "role",
        "deformable",
        "visibleInScene",
        "locked",
        "collapsed",
        "markerColour",
        "backgroundColour",
        "columnWidth",
        "createdAt",
      ],
      allOf: [
        {
          if: {
            properties: {
              elementType: {
                const: "drawing",
              },
            },
            required: [
              "elementType",
            ],
          },
          then: {
            required: [
              "drawingBankId",
              "exposure",
              "transform",
              "channels",
              "visibleInXsheet",
            ],
            properties: {
              importOptions: false,
              streamed: false,
              showMouthShapes: false,
              placements: false,
              lipSyncMappings: false,
              sequences: false,
              exportGroup: false,
              camera: false,
              skeleton: false,
              bone: false,
              slot: false,
              params: false,
              blocks: false,
              transform: {
                required: [
                  "rotationPivot",
                  "scalePivot",
                  "rotateAboutPivot",
                ],
                properties: {
                  pivot: false,
                  applyPivotTo: false,
                  useChildDrawingPivot: false,
                },
              },
              channels: {
                propertyNames: {
                  enum: [
                    "position",
                    "velocity",
                    "pivot.x",
                    "pivot.y",
                    "rotation",
                    "scale.x",
                    "scale.y",
                    "skew",
                  ],
                },
              },
            },
          },
        },
        {
          if: {
            properties: {
              elementType: {
                const: "annotation",
              },
            },
            required: [
              "elementType",
            ],
          },
          then: {
            required: [
              "drawingBankId",
              "exposure",
              "transform",
              "channels",
              "visibleInXsheet",
            ],
            properties: {
              importOptions: false,
              streamed: false,
              showMouthShapes: false,
              placements: false,
              lipSyncMappings: false,
              sequences: false,
              exportGroup: false,
              camera: false,
              skeleton: false,
              bone: false,
              slot: false,
              params: false,
              blocks: false,
              transform: {
                required: [
                  "rotationPivot",
                  "scalePivot",
                  "rotateAboutPivot",
                ],
                properties: {
                  pivot: false,
                  applyPivotTo: false,
                  useChildDrawingPivot: false,
                },
              },
              channels: {
                propertyNames: {
                  enum: [
                    "position",
                    "velocity",
                    "pivot.x",
                    "pivot.y",
                    "rotation",
                    "scale.x",
                    "scale.y",
                    "skew",
                  ],
                },
              },
            },
          },
        },
        {
          if: {
            properties: {
              elementType: {
                const: "image",
              },
            },
            required: [
              "elementType",
            ],
          },
          then: {
            required: [
              "drawingBankId",
              "exposure",
              "importOptions",
              "transform",
              "channels",
              "visibleInXsheet",
            ],
            properties: {
              streamed: false,
              showMouthShapes: false,
              placements: false,
              lipSyncMappings: false,
              sequences: false,
              exportGroup: false,
              camera: false,
              skeleton: false,
              bone: false,
              slot: false,
              params: false,
              blocks: false,
              transform: {
                required: [
                  "rotationPivot",
                  "scalePivot",
                  "rotateAboutPivot",
                ],
                properties: {
                  pivot: false,
                  applyPivotTo: false,
                  useChildDrawingPivot: false,
                },
              },
              channels: {
                propertyNames: {
                  enum: [
                    "position",
                    "velocity",
                    "pivot.x",
                    "pivot.y",
                    "rotation",
                    "scale.x",
                    "scale.y",
                    "skew",
                  ],
                },
              },
            },
          },
        },
        {
          if: {
            properties: {
              elementType: {
                const: "sound",
              },
            },
            required: [
              "elementType",
            ],
          },
          then: {
            required: [
              "streamed",
              "showMouthShapes",
              "placements",
              "lipSyncMappings",
              "visibleInXsheet",
            ],
            properties: {
              drawingBankId: false,
              exposure: false,
              cellNotes: false,
              importOptions: false,
              sequences: false,
              transform: false,
              exportGroup: false,
              camera: false,
              skeleton: false,
              bone: false,
              slot: false,
              params: false,
              blocks: false,
              channels: false,
            },
          },
        },
        {
          if: {
            properties: {
              elementType: {
                const: "media",
              },
            },
            required: [
              "elementType",
            ],
          },
          then: {
            required: [
              "sequences",
              "transform",
              "channels",
              "visibleInXsheet",
            ],
            properties: {
              drawingBankId: false,
              exposure: false,
              cellNotes: false,
              importOptions: false,
              streamed: false,
              showMouthShapes: false,
              placements: false,
              lipSyncMappings: false,
              exportGroup: false,
              camera: false,
              skeleton: false,
              bone: false,
              slot: false,
              params: false,
              blocks: false,
              transform: {
                required: [
                  "rotationPivot",
                  "scalePivot",
                  "rotateAboutPivot",
                ],
                properties: {
                  pivot: false,
                  applyPivotTo: false,
                  useChildDrawingPivot: false,
                },
              },
              channels: {
                propertyNames: {
                  enum: [
                    "position",
                    "velocity",
                    "pivot.x",
                    "pivot.y",
                    "rotation",
                    "scale.x",
                    "scale.y",
                    "skew",
                  ],
                },
              },
            },
          },
        },
        {
          if: {
            properties: {
              elementType: {
                const: "peg",
              },
            },
            required: [
              "elementType",
            ],
          },
          then: {
            required: [
              "transform",
              "exportGroup",
              "channels",
              "start",
              "duration",
            ],
            properties: {
              drawingBankId: false,
              exposure: false,
              cellNotes: false,
              importOptions: false,
              streamed: false,
              showMouthShapes: false,
              placements: false,
              lipSyncMappings: false,
              sequences: false,
              camera: false,
              skeleton: false,
              bone: false,
              slot: false,
              params: false,
              blocks: false,
              visibleInXsheet: false,
              transform: {
                required: [
                  "pivot",
                  "applyPivotTo",
                  "useChildDrawingPivot",
                ],
                properties: {
                  rotationPivot: false,
                  scalePivot: false,
                  rotateAboutPivot: false,
                },
              },
              channels: {
                propertyNames: {
                  enum: [
                    "position",
                    "velocity",
                    "pivot.x",
                    "pivot.y",
                    "rotation",
                    "scale.x",
                    "scale.y",
                    "skew",
                  ],
                },
              },
            },
          },
        },
        {
          if: {
            properties: {
              elementType: {
                const: "camera",
              },
            },
            required: [
              "elementType",
            ],
          },
          then: {
            required: [
              "camera",
              "transform",
              "channels",
              "start",
              "duration",
            ],
            properties: {
              drawingBankId: false,
              exposure: false,
              cellNotes: false,
              importOptions: false,
              streamed: false,
              showMouthShapes: false,
              placements: false,
              lipSyncMappings: false,
              sequences: false,
              exportGroup: false,
              skeleton: false,
              bone: false,
              slot: false,
              params: false,
              blocks: false,
              visibleInXsheet: false,
              transform: {
                required: [],
                properties: {
                  rotationPivot: false,
                  scalePivot: false,
                  rotateAboutPivot: false,
                  pivot: false,
                  applyPivotTo: false,
                  useChildDrawingPivot: false,
                },
              },
              channels: {
                propertyNames: {
                  enum: [
                    "position",
                    "velocity",
                    "rotation",
                    "fov",
                  ],
                },
              },
            },
          },
        },
        {
          if: {
            properties: {
              elementType: {
                const: "skeleton",
              },
            },
            required: [
              "elementType",
            ],
          },
          then: {
            required: [
              "skeleton",
              "transform",
              "channels",
              "start",
              "duration",
            ],
            properties: {
              drawingBankId: false,
              exposure: false,
              cellNotes: false,
              importOptions: false,
              streamed: false,
              showMouthShapes: false,
              placements: false,
              lipSyncMappings: false,
              sequences: false,
              exportGroup: false,
              camera: false,
              bone: false,
              slot: false,
              params: false,
              blocks: false,
              visibleInXsheet: false,
              transform: {
                required: [
                  "pivot",
                  "applyPivotTo",
                ],
                properties: {
                  rotationPivot: false,
                  scalePivot: false,
                  rotateAboutPivot: false,
                  useChildDrawingPivot: false,
                },
              },
              channels: {
                propertyNames: {
                  enum: [
                    "position",
                    "velocity",
                    "pivot.x",
                    "pivot.y",
                    "rotation",
                    "scale.x",
                    "scale.y",
                    "skew",
                  ],
                },
              },
            },
          },
        },
        {
          if: {
            properties: {
              elementType: {
                const: "bone",
              },
            },
            required: [
              "elementType",
            ],
          },
          then: {
            required: [
              "bone",
              "channels",
              "start",
              "duration",
            ],
            properties: {
              drawingBankId: false,
              exposure: false,
              cellNotes: false,
              importOptions: false,
              streamed: false,
              showMouthShapes: false,
              placements: false,
              lipSyncMappings: false,
              sequences: false,
              transform: false,
              exportGroup: false,
              camera: false,
              skeleton: false,
              slot: false,
              params: false,
              blocks: false,
              visibleInXsheet: false,
              channels: {
                propertyNames: {
                  enum: [
                    "rotation",
                    "bone.x",
                    "bone.y",
                    "bone.length",
                  ],
                },
              },
            },
          },
        },
        {
          if: {
            properties: {
              elementType: {
                const: "effectParameter",
              },
            },
            required: [
              "elementType",
            ],
          },
          then: {
            required: [
              "slot",
              "start",
              "duration",
            ],
            properties: {
              drawingBankId: false,
              exposure: false,
              cellNotes: false,
              importOptions: false,
              streamed: false,
              showMouthShapes: false,
              placements: false,
              lipSyncMappings: false,
              sequences: false,
              transform: false,
              exportGroup: false,
              camera: false,
              skeleton: false,
              bone: false,
              params: false,
              blocks: false,
              channels: false,
              visibleInXsheet: false,
            },
          },
        },
        {
          if: {
            properties: {
              elementType: {
                const: "colourTransform",
              },
            },
            required: [
              "elementType",
            ],
          },
          then: {
            required: [
              "params",
              "channels",
              "start",
              "duration",
            ],
            properties: {
              drawingBankId: false,
              exposure: false,
              cellNotes: false,
              importOptions: false,
              streamed: false,
              showMouthShapes: false,
              placements: false,
              lipSyncMappings: false,
              sequences: false,
              transform: false,
              exportGroup: false,
              camera: false,
              skeleton: false,
              bone: false,
              slot: false,
              blocks: false,
              visibleInXsheet: false,
              channels: {
                propertyNames: {
                  enum: [
                    "multRed",
                    "multGreen",
                    "multBlue",
                    "multAlpha",
                    "addRed",
                    "addGreen",
                    "addBlue",
                    "addAlpha",
                  ],
                },
              },
              params: {
                $ref: "#/$defs/ColourTransformParams",
              },
            },
          },
        },
        {
          if: {
            properties: {
              elementType: {
                const: "clipping",
              },
            },
            required: [
              "elementType",
            ],
          },
          then: {
            required: [
              "params",
              "channels",
              "start",
              "duration",
            ],
            properties: {
              drawingBankId: false,
              exposure: false,
              cellNotes: false,
              importOptions: false,
              streamed: false,
              showMouthShapes: false,
              placements: false,
              lipSyncMappings: false,
              sequences: false,
              transform: false,
              exportGroup: false,
              camera: false,
              skeleton: false,
              bone: false,
              slot: false,
              blocks: false,
              visibleInXsheet: false,
              channels: {
                propertyNames: {
                  enum: [],
                },
              },
              params: {
                $ref: "#/$defs/ClippingParams",
              },
            },
          },
        },
        {
          if: {
            properties: {
              elementType: {
                const: "dropShadow",
              },
            },
            required: [
              "elementType",
            ],
          },
          then: {
            required: [
              "params",
              "channels",
              "start",
              "duration",
            ],
            properties: {
              drawingBankId: false,
              exposure: false,
              cellNotes: false,
              importOptions: false,
              streamed: false,
              showMouthShapes: false,
              placements: false,
              lipSyncMappings: false,
              sequences: false,
              transform: false,
              exportGroup: false,
              camera: false,
              skeleton: false,
              bone: false,
              slot: false,
              blocks: false,
              visibleInXsheet: false,
              channels: {
                propertyNames: {
                  enum: [],
                },
              },
              params: {
                $ref: "#/$defs/DropShadowParams",
              },
            },
          },
        },
        {
          if: {
            properties: {
              elementType: {
                const: "blur",
              },
            },
            required: [
              "elementType",
            ],
          },
          then: {
            required: [
              "params",
              "channels",
              "start",
              "duration",
            ],
            properties: {
              drawingBankId: false,
              exposure: false,
              cellNotes: false,
              importOptions: false,
              streamed: false,
              showMouthShapes: false,
              placements: false,
              lipSyncMappings: false,
              sequences: false,
              transform: false,
              exportGroup: false,
              camera: false,
              skeleton: false,
              bone: false,
              slot: false,
              blocks: false,
              visibleInXsheet: false,
              channels: {
                propertyNames: {
                  enum: [
                    "radius",
                    "angle",
                  ],
                },
              },
              params: {
                $ref: "#/$defs/BlurParams",
              },
            },
          },
        },
        {
          if: {
            properties: {
              elementType: {
                const: "shake",
              },
            },
            required: [
              "elementType",
            ],
          },
          then: {
            required: [
              "params",
              "channels",
              "start",
              "duration",
            ],
            properties: {
              drawingBankId: false,
              exposure: false,
              cellNotes: false,
              importOptions: false,
              streamed: false,
              showMouthShapes: false,
              placements: false,
              lipSyncMappings: false,
              sequences: false,
              transform: false,
              exportGroup: false,
              camera: false,
              skeleton: false,
              bone: false,
              slot: false,
              blocks: false,
              visibleInXsheet: false,
              channels: {
                propertyNames: {
                  enum: [
                    "amplitude",
                  ],
                },
              },
              params: {
                $ref: "#/$defs/ShakeParams",
              },
            },
          },
        },
        {
          if: {
            properties: {
              elementType: {
                const: "particleNature",
              },
            },
            required: [
              "elementType",
            ],
          },
          then: {
            required: [
              "params",
              "blocks",
              "start",
              "duration",
            ],
            properties: {
              drawingBankId: false,
              exposure: false,
              cellNotes: false,
              importOptions: false,
              streamed: false,
              showMouthShapes: false,
              placements: false,
              lipSyncMappings: false,
              sequences: false,
              transform: false,
              exportGroup: false,
              camera: false,
              skeleton: false,
              bone: false,
              slot: false,
              channels: false,
              visibleInXsheet: false,
              params: {
                $ref: "#/$defs/ParticleParams",
              },
              blocks: {
                items: {
                  properties: {
                    particleType: {
                      enum: [
                        "rain",
                        "snow",
                        "hail",
                        "fog",
                        "cloud",
                        "eruption",
                      ],
                    },
                  },
                },
              },
            },
          },
        },
        {
          if: {
            properties: {
              elementType: {
                const: "particleImpact",
              },
            },
            required: [
              "elementType",
            ],
          },
          then: {
            required: [
              "params",
              "blocks",
              "start",
              "duration",
            ],
            properties: {
              drawingBankId: false,
              exposure: false,
              cellNotes: false,
              importOptions: false,
              streamed: false,
              showMouthShapes: false,
              placements: false,
              lipSyncMappings: false,
              sequences: false,
              transform: false,
              exportGroup: false,
              camera: false,
              skeleton: false,
              bone: false,
              slot: false,
              channels: false,
              visibleInXsheet: false,
              params: {
                $ref: "#/$defs/ParticleParams",
              },
              blocks: {
                items: {
                  properties: {
                    particleType: {
                      enum: [
                        "shatter",
                        "magic",
                        "explosion",
                        "splash",
                        "fire",
                        "celestial",
                      ],
                    },
                  },
                },
              },
            },
          },
        },
      ],
    },
    else: {
      propertyNames: {
        enum: [
          "id",
          "type",
          "name",
        ],
      },
    },
  },
  ElementType: {
    oneOf: [
      {
        type: "string",
        const: "drawing",
        description: "Vector drawing element.",
      },
      {
        type: "string",
        const: "annotation",
        description: "Annotation (note drawing) element.",
      },
      {
        type: "string",
        const: "image",
        description: "Bitmap image element.",
      },
      {
        type: "string",
        const: "sound",
        description: "Sound element.",
      },
      {
        type: "string",
        const: "media",
        description: "Media element (linked sources).",
      },
      {
        type: "string",
        const: "peg",
        description: "Peg.",
      },
      {
        type: "string",
        const: "camera",
        description: "Camera.",
      },
      {
        type: "string",
        const: "skeleton",
        description: "Skeleton effect.",
      },
      {
        type: "string",
        const: "bone",
        description: "Bone.",
      },
      {
        type: "string",
        const: "effectParameter",
        description: "Effect parameter element (mask, sprites, bones, skin slot).",
      },
      {
        type: "string",
        const: "colourTransform",
        description: "Colour transform effect.",
      },
      {
        type: "string",
        const: "clipping",
        description: "Clipping effect.",
      },
      {
        type: "string",
        const: "dropShadow",
        description: "Drop shadow effect.",
      },
      {
        type: "string",
        const: "blur",
        description: "Blur effect.",
      },
      {
        type: "string",
        const: "shake",
        description: "Shake effect.",
      },
      {
        type: "string",
        const: "particleNature",
        description: "Natural-phenomena particle effect.",
      },
      {
        type: "string",
        const: "particleImpact",
        description: "Action/impact particle effect.",
      },
    ],
    description: "Element types (10 §4.5). The 04 read model reports this value as `type`; the object model uses `elementType`\n(design decision D6).",
  },
  EnvelopePoint: {
    type: "object",
    additionalProperties: false,
    properties: {
      time: {
        type: "number",
        "x-unit": "second",
        description: "Time within the source (seconds).",
        minimum: 0,
        "x-since": "1.0",
      },
      left: {
        type: "number",
        maximum: 1,
        description: "Left gain 0–1.",
        minimum: 0,
        "x-since": "1.0",
      },
      right: {
        type: "number",
        maximum: 1,
        description: "Right gain 0–1.",
        minimum: 0,
        "x-since": "1.0",
      },
    },
    required: [
      "time",
      "left",
      "right",
    ],
    description: "A gain point of a placement envelope (07 §9.5).",
  },
  ExportGroup: {
    type: "object",
    additionalProperties: false,
    properties: {
      groupOnVectorExport: {
        type: "boolean",
        description: "Group the subtree on vector export.",
        "x-since": "1.0",
      },
      addComposite: {
        type: "boolean",
        description: "Add a composite for the export group.",
        "x-since": "1.0",
      },
    },
    required: [
      "groupOnVectorExport",
      "addComposite",
    ],
    description: "Export-group block of pegs (05 §8.2, 09).",
  },
  Ext: {
    type: "object",
    propertyNames: {
      pattern: "^[A-Za-z0-9][A-Za-z0-9-]*(\\.[A-Za-z0-9][A-Za-z0-9-]*)+$",
    },
    description: "Extension data keyed by reverse-DNS namespace; unknown namespaces are preserved verbatim.",
  },
  FovMode: {
    oneOf: [
      {
        type: "string",
        const: "static",
        description: "A single FOV value.",
      },
      {
        type: "string",
        const: "dynamic",
        description: "The `fov` channel.",
      },
    ],
    description: "Camera FOV mode (05 §12.3).",
  },
  FrameRange: {
    type: "object",
    additionalProperties: false,
    properties: {
      from: {
        type: "integer",
        maximum: 2147483647,
        minimum: 1,
        description: "First frame (1-based).",
        "x-since": "1.0",
      },
      to: {
        type: "integer",
        maximum: 2147483647,
        minimum: 1,
        description: "Last frame (inclusive).",
        "x-since": "1.0",
      },
    },
    required: [
      "from",
      "to",
    ],
    description: "An inclusive frame range `{from, to}` (10 §1.1).",
  },
  FrameValue: {
    type: "object",
    additionalProperties: false,
    properties: {
      frame: {
        type: "integer",
        maximum: 2147483647,
        minimum: 1,
        description: "Absolute scene frame.",
        "x-unit": "frame",
        "x-since": "1.0",
      },
      value: {
        type: "number",
        description: "Value.",
        "x-since": "1.0",
      },
    },
    required: [
      "frame",
      "value",
    ],
    description: "A `{frame, value}` keyframe of a dynamic block parameter.",
  },
  GradientStop: {
    type: "object",
    additionalProperties: false,
    properties: {
      position: {
        type: "number",
        maximum: 1,
        description: "Position along the gradient, 0–1.",
        minimum: 0,
        "x-since": "1.0",
      },
      rgba: {
        $ref: "#/$defs/Colour",
        description: "Colour at the stop.",
        "x-since": "1.0",
      },
    },
    required: [
      "position",
      "rgba",
    ],
    description: "A gradient stop (03 §12.3).",
  },
  GradientValue: {
    type: "object",
    additionalProperties: false,
    properties: {
      stops: {
        type: "array",
        items: {
          $ref: "#/$defs/GradientStop",
        },
        description: "The stops.",
        minItems: 2,
        "x-since": "1.0",
      },
    },
    required: [
      "stops",
    ],
    description: "`{stops}` with at least two stops sorted by position.",
  },
  HandleMode: {
    oneOf: [
      {
        type: "string",
        const: "smooth",
        description: "Collinear handles (default).",
      },
      {
        type: "string",
        const: "broken",
        description: "Independent handles.",
      },
    ],
    description: "Handle mode.",
  },
  HandleOffset: {
    type: "object",
    additionalProperties: false,
    properties: {
      dt: {
        type: "number",
        "x-unit": "frame",
        description: "Frame offset.",
        "x-since": "1.0",
      },
      dv: {
        type: "number",
        description: "Value offset.",
        "x-since": "1.0",
      },
    },
    required: [
      "dt",
      "dv",
    ],
    description: "A handle offset `{dt, dv}` (05 §10.3).",
  },
  Handles: {
    type: "object",
    additionalProperties: false,
    properties: {
      left: {
        $ref: "#/$defs/HandleOffset",
        description: "Left handle.",
        "x-since": "1.0",
      },
      right: {
        $ref: "#/$defs/HandleOffset",
        description: "Right handle.",
        "x-since": "1.0",
      },
      mode: {
        $ref: "#/$defs/HandleMode",
        description: "Smooth or broken.",
        "x-since": "1.0",
      },
    },
    required: [
      "left",
      "right",
      "mode",
    ],
    description: "Handles of a key.",
  },
  Id: {
    type: "string",
    pattern: "^[A-Za-z0-9_-]{1,64}$",
  },
  InfluenceType: {
    oneOf: [
      {
        type: "string",
        const: "infinite",
        description: "Unlimited (default).",
      },
      {
        type: "string",
        const: "elliptical",
        description: "Elliptical zone.",
      },
    ],
    description: "Influence type of a bone.",
  },
  ItemKind: {
    oneOf: [
      {
        type: "string",
        const: "template",
        description: "A template (`.animtpl`).",
      },
      {
        type: "string",
        const: "image",
        description: "An image file.",
      },
      {
        type: "string",
        const: "movie",
        description: "A movie file.",
      },
      {
        type: "string",
        const: "sound",
        description: "A sound file.",
      },
    ],
    description: "Kind of a library item (10 §4.17).",
  },
  Key: {
    type: "object",
    additionalProperties: false,
    properties: {
      id: {
        $ref: "#/$defs/Id",
        description: "ID, unique within the channel (ID-03).",
        readOnly: true,
        "x-since": "1.0",
      },
      type: {
        type: "string",
        const: "key",
        readOnly: true,
        description: "Always `key`.",
        "x-since": "1.0",
      },
      rev: {
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
        description: "Revision of the last change.",
        readOnly: true,
        "x-since": "1.0",
      },
      frame: {
        type: "integer",
        maximum: 2147483647,
        minimum: 1,
        description: "Integer frame, unique in the channel.",
        "x-unit": "frame",
        "x-since": "1.0",
      },
      value: {
        type: "number",
        description: "Value.",
        "x-since": "1.0",
      },
      constantSegment: {
        type: "boolean",
        description: "The segment starting here holds the value.",
        "x-since": "1.0",
      },
      handles: {
        $ref: "#/$defs/Handles",
        description: "Spline handles.",
        "x-since": "1.0",
      },
      easing: {
        anyOf: [
          {
            $ref: "#/$defs/Easing",
          },
          {
            type: "null",
          },
        ],
        description: "Easing preset of the segment starting here (interpolation curves).",
        "x-since": "1.0",
      },
      meta: {
        type: "object",
        additionalProperties: true,
        description: "Free-form metadata.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      ext: {
        $ref: "#/$defs/Ext",
        description: "Extension data.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      path: {
        type: "string",
        description: "Canonical path (10 §2.4).",
        "x-transient": true,
        readOnly: true,
        "x-since": "1.0",
      },
    },
    "x-since": "1.0",
    description: "A key of a 1-D channel (10 §4.12). Addressed as `key:\"<frame>\"` or `key:@<id>`.",
    "x-objectType": "key",
    required: [
      "id",
      "type",
    ],
    if: {
      required: [
        "rev",
      ],
    },
    then: {
      required: [
        "id",
        "type",
        "rev",
        "frame",
        "value",
        "constantSegment",
        "handles",
        "easing",
      ],
    },
    else: {
      propertyNames: {
        enum: [
          "id",
          "type",
          "name",
        ],
      },
    },
  },
  LayerHint: {
    oneOf: [
      {
        type: "string",
        const: "brush",
        description: "Contour artwork.",
      },
      {
        type: "string",
        const: "pencil",
        description: "Centre-line artwork.",
      },
    ],
    description: "Type hint of a layer (02 §3.1).",
  },
  Library: {
    type: "object",
    additionalProperties: false,
    properties: {
      id: {
        $ref: "#/$defs/Id",
        description: "Immutable, globally unique ID (08 LIB-002).",
        readOnly: true,
        "x-since": "1.0",
      },
      type: {
        type: "string",
        const: "library",
        readOnly: true,
        description: "Always `library`.",
        "x-since": "1.0",
      },
      rev: {
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
        description: "Revision of the last change.",
        readOnly: true,
        "x-since": "1.0",
      },
      name: {
        pattern: "^(?:[^\\u0000-\\u001F\\u007F\\\\/:*?\"<>|\\s]|[^\\u0000-\\u001F\\u007F\\\\/:*?\"<>|\\s][^\\u0000-\\u001F\\u007F\\\\/:*?\"<>|]*[^\\u0000-\\u001F\\u007F\\\\/:*?\"<>|\\s])$",
        description: "Library name.",
        minLength: 1,
        maxLength: 128,
        type: "string",
        "x-since": "1.0",
      },
      kind: {
        $ref: "#/$defs/LibraryKind",
        description: "Local, global, opened or starter.",
        readOnly: true,
        "x-since": "1.0",
      },
      formatVersion: {
        type: "string",
        description: "Library format version (`MAJOR.MINOR`).",
        pattern: "^[0-9]+\\.[0-9]+$",
        readOnly: true,
        "x-since": "1.0",
      },
      children: {
        type: "array",
        items: {
          $ref: "#/$defs/LibraryNode",
        },
        description: "Top-level folders and items.",
        "x-since": "1.0",
      },
      meta: {
        type: "object",
        additionalProperties: true,
        description: "Free-form metadata.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      ext: {
        $ref: "#/$defs/Ext",
        description: "Extension data.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      path: {
        type: "string",
        description: "Canonical path (10 §2.4).",
        "x-transient": true,
        readOnly: true,
        "x-since": "1.0",
      },
      readOnly: {
        type: "boolean",
        "x-transient": true,
        readOnly: true,
        "x-since": "1.0",
      },
      writeAccessHeldBy: {
        type: [
          "string",
          "null",
        ],
        "x-transient": true,
        readOnly: true,
        "x-since": "1.0",
      },
      location: {
        type: [
          "string",
          "null",
        ],
        "x-transient": true,
        readOnly: true,
        "x-since": "1.0",
      },
    },
    "x-since": "1.0",
    description: "A library (10 §4.17). In the document this is the project-local library (`kind: \"local\"`), stored under\n`library/library.json` in the package layout of 10 §6.11.",
    "x-objectType": "library",
    required: [
      "id",
      "type",
    ],
    if: {
      required: [
        "rev",
      ],
    },
    then: {
      required: [
        "id",
        "type",
        "rev",
        "name",
        "kind",
        "formatVersion",
        "children",
      ],
    },
    else: {
      propertyNames: {
        enum: [
          "id",
          "type",
          "name",
        ],
      },
    },
  },
  LibraryFolder: {
    type: "object",
    additionalProperties: false,
    properties: {
      id: {
        $ref: "#/$defs/Id",
        description: "Immutable, globally unique ID (08 LIB-002).",
        readOnly: true,
        "x-since": "1.0",
      },
      type: {
        type: "string",
        const: "libraryFolder",
        readOnly: true,
        description: "Always `libraryFolder`.",
        "x-since": "1.0",
      },
      rev: {
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
        description: "Revision of the last change.",
        readOnly: true,
        "x-since": "1.0",
      },
      name: {
        pattern: "^(?:[^\\u0000-\\u001F\\u007F\\\\/:*?\"<>|\\s]|[^\\u0000-\\u001F\\u007F\\\\/:*?\"<>|\\s][^\\u0000-\\u001F\\u007F\\\\/:*?\"<>|]*[^\\u0000-\\u001F\\u007F\\\\/:*?\"<>|\\s])$",
        description: "Unique among siblings (case-insensitive).",
        "x-command": "library.folder.rename",
        minLength: 1,
        maxLength: 128,
        type: "string",
        "x-since": "1.0",
      },
      children: {
        type: "array",
        items: {
          $ref: "#/$defs/LibraryNode",
        },
        description: "Sub-folders and items.",
        "x-since": "1.0",
      },
      meta: {
        type: "object",
        additionalProperties: true,
        description: "Free-form metadata.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      ext: {
        $ref: "#/$defs/Ext",
        description: "Extension data.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      path: {
        type: "string",
        description: "Canonical path (10 §2.4).",
        "x-transient": true,
        readOnly: true,
        "x-since": "1.0",
      },
    },
    "x-since": "1.0",
    description: "A library folder (10 §4.17).",
    "x-objectType": "libraryFolder",
    required: [
      "id",
      "type",
    ],
    if: {
      required: [
        "rev",
      ],
    },
    then: {
      required: [
        "id",
        "type",
        "rev",
        "name",
        "children",
      ],
    },
    else: {
      propertyNames: {
        enum: [
          "id",
          "type",
          "name",
        ],
      },
    },
  },
  LibraryItem: {
    type: "object",
    additionalProperties: false,
    properties: {
      id: {
        $ref: "#/$defs/Id",
        description: "Immutable, globally unique ID (08 LIB-002).",
        readOnly: true,
        "x-since": "1.0",
      },
      type: {
        type: "string",
        const: "libraryItem",
        readOnly: true,
        description: "Always `libraryItem`.",
        "x-since": "1.0",
      },
      rev: {
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
        description: "Revision of the last change.",
        readOnly: true,
        "x-since": "1.0",
      },
      name: {
        pattern: "^(?:[^\\u0000-\\u001F\\u007F\\\\/:*?\"<>|\\s]|[^\\u0000-\\u001F\\u007F\\\\/:*?\"<>|\\s][^\\u0000-\\u001F\\u007F\\\\/:*?\"<>|]*[^\\u0000-\\u001F\\u007F\\\\/:*?\"<>|\\s])$",
        description: "Display name, unique among siblings (case-insensitive).",
        "x-command": "library.item.rename",
        minLength: 1,
        maxLength: 128,
        type: "string",
        "x-since": "1.0",
      },
      itemKind: {
        $ref: "#/$defs/ItemKind",
        description: "Template, image, movie or sound.",
        readOnly: true,
        "x-since": "1.0",
      },
      file: {
        type: "string",
        description: "Stored file name relative to the library's `files/` folder.",
        readOnly: true,
        "x-since": "1.0",
      },
      thumbnail: {
        anyOf: [
          {
            $ref: "#/$defs/Blob",
          },
          {
            type: "null",
          },
        ],
        description: "Thumbnail, if any.",
        "x-since": "1.0",
      },
      versionId: {
        type: "string",
        description: "Templates: version ID, changed whenever the content changes (08 LIB-007).",
        "x-kindSpecific": true,
        readOnly: true,
        "x-since": "1.0",
      },
      artist: {
        type: "string",
        description: "Templates: artist (08 LIB-010).",
        "x-kindSpecific": true,
        "x-since": "1.0",
      },
      copyright: {
        type: "string",
        description: "Templates: copyright.",
        "x-kindSpecific": true,
        "x-since": "1.0",
      },
      createdAt: {
        readOnly: true,
        description: "Templates: creation time.",
        "x-kindSpecific": true,
        $ref: "#/$defs/Timestamp",
        "x-since": "1.0",
      },
      modifiedAt: {
        readOnly: true,
        description: "Templates: modification time.",
        "x-kindSpecific": true,
        $ref: "#/$defs/Timestamp",
        "x-since": "1.0",
      },
      sizeBytes: {
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
        description: "Templates: size in bytes.",
        "x-kindSpecific": true,
        readOnly: true,
        "x-since": "1.0",
      },
      formatVersion: {
        type: "string",
        description: "Templates: template format version.",
        "x-kindSpecific": true,
        readOnly: true,
        "x-since": "1.0",
      },
      source: {
        type: "string",
        description: "Templates: source path or link (optional).",
        "x-kindSpecific": true,
        "x-since": "1.0",
      },
      summary: {
        type: "array",
        items: {
          $ref: "#/$defs/TemplateSummaryNode",
        },
        description: "Templates: element-tree summary.",
        "x-kindSpecific": true,
        readOnly: true,
        "x-since": "1.0",
      },
      meta: {
        type: "object",
        additionalProperties: true,
        description: "Free-form metadata.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      ext: {
        $ref: "#/$defs/Ext",
        description: "Extension data.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      path: {
        type: "string",
        description: "Canonical path (10 §2.4).",
        "x-transient": true,
        readOnly: true,
        "x-since": "1.0",
      },
      broken: {
        type: "boolean",
        "x-transient": true,
        readOnly: true,
        "x-since": "1.0",
      },
    },
    "x-since": "1.0",
    description: "A library item (10 §4.17). Template metadata (08 §6.3) is present for templates only.",
    "x-objectType": "libraryItem",
    required: [
      "id",
      "type",
    ],
    if: {
      required: [
        "rev",
      ],
    },
    then: {
      required: [
        "id",
        "type",
        "rev",
        "name",
        "itemKind",
        "file",
        "thumbnail",
      ],
      allOf: [
        {
          if: {
            properties: {
              itemKind: {
                const: "template",
              },
            },
            required: [
              "itemKind",
            ],
          },
          then: {
            required: [
              "versionId",
              "artist",
              "copyright",
              "createdAt",
              "modifiedAt",
              "sizeBytes",
              "formatVersion",
              "summary",
            ],
          },
          else: {
            properties: {
              versionId: false,
              artist: false,
              copyright: false,
              createdAt: false,
              modifiedAt: false,
              sizeBytes: false,
              formatVersion: false,
              summary: false,
              source: false,
            },
          },
        },
      ],
    },
    else: {
      propertyNames: {
        enum: [
          "id",
          "type",
          "name",
        ],
      },
    },
  },
  LibraryKind: {
    oneOf: [
      {
        type: "string",
        const: "local",
        description: "The project-local library.",
      },
      {
        type: "string",
        const: "global",
        description: "The user's global library.",
      },
      {
        type: "string",
        const: "opened",
        description: "An opened library folder.",
      },
      {
        type: "string",
        const: "starter",
        description: "The starter library.",
      },
    ],
    description: "Kind of a library (10 §4.17).",
  },
  LibraryNode: {
    oneOf: [
      {
        $ref: "#/$defs/LibraryFolder",
      },
      {
        $ref: "#/$defs/LibraryItem",
      },
    ],
  },
  LineStyle: {
    type: "object",
    additionalProperties: false,
    properties: {
      thickness: {
        type: "number",
        "x-unit": "du",
        description: "Thickness in drawing units.",
        minimum: 0,
        "x-since": "1.0",
      },
      lineType: {
        $ref: "#/$defs/LineType",
        description: "Line type.",
        "x-since": "1.0",
      },
    },
    required: [
      "thickness",
      "lineType",
    ],
    description: "Line style of a pencil stroke.",
  },
  LineType: {
    oneOf: [
      {
        type: "string",
        const: "solid",
        description: "Solid line.",
      },
    ],
    description: "Line type of a pencil line style (02 §3.2; other types MAY be added).",
  },
  LinkKind: {
    oneOf: [
      {
        type: "string",
        const: "file",
        description: "An external file.",
      },
      {
        type: "string",
        const: "libraryItem",
        description: "A library item (08 LIB-002).",
      },
    ],
    description: "Kind of a link.",
  },
  LipSyncMapping: {
    type: "object",
    additionalProperties: false,
    properties: {
      drawingElementId: {
        $ref: "#/$defs/Id",
        description: "The mapped drawing element.",
        "x-ref": "element",
        "x-since": "1.0",
      },
      mapping: {
        $ref: "#/$defs/MouthMapping",
        description: "Shape → drawing.",
        "x-since": "1.0",
      },
      autoReapply: {
        type: "boolean",
        description: "Re-apply automatically when the track changes.",
        "x-since": "1.0",
      },
    },
    required: [
      "drawingElementId",
      "mapping",
      "autoReapply",
    ],
    description: "A lip-sync mapping for one drawing element (10 §4.14).",
  },
  LoopType: {
    oneOf: [
      {
        type: "string",
        const: "count",
        description: "The duration becomes `loops × L`.",
      },
      {
        type: "string",
        const: "fill",
        description: "The cycle length is kept; cycles follow the duration.",
      },
    ],
    description: "Loop type (05 §8.4).",
  },
  MediaSequence: {
    type: "object",
    additionalProperties: false,
    properties: {
      id: {
        $ref: "#/$defs/Id",
        description: "Immutable ID.",
        readOnly: true,
        "x-since": "1.0",
      },
      type: {
        type: "string",
        const: "mediaSequence",
        readOnly: true,
        description: "Always `mediaSequence`.",
        "x-since": "1.0",
      },
      rev: {
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
        description: "Revision of the last change.",
        readOnly: true,
        "x-since": "1.0",
      },
      assetId: {
        $ref: "#/$defs/Id",
        description: "A `linked` asset.",
        "x-ref": "asset",
        "x-since": "1.0",
      },
      startFrame: {
        type: "integer",
        maximum: 2147483647,
        minimum: 1,
        description: "First frame.",
        "x-unit": "frame",
        "x-since": "1.0",
      },
      stopFrame: {
        type: "integer",
        maximum: 2147483647,
        minimum: 1,
        description: "Last frame.",
        "x-unit": "frame",
        "x-since": "1.0",
      },
      linkOffset: {
        type: "integer",
        maximum: 2147483647,
        minimum: 1,
        description: "Frame of the source shown at `startFrame`, ≥ 1.",
        "x-unit": "frame",
        "x-since": "1.0",
      },
      meta: {
        type: "object",
        additionalProperties: true,
        description: "Free-form metadata.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      ext: {
        $ref: "#/$defs/Ext",
        description: "Extension data.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      path: {
        type: "string",
        description: "Canonical path (10 §2.4).",
        "x-transient": true,
        readOnly: true,
        "x-since": "1.0",
      },
      sourceLength: {
        type: "integer",
        minimum: 0,
        maximum: 2147483647,
        "x-derived": true,
        readOnly: true,
        "x-since": "1.0",
      },
      missing: {
        type: "boolean",
        "x-derived": true,
        readOnly: true,
        "x-since": "1.0",
      },
    },
    "x-since": "1.0",
    description: "A run of frames of a media element showing one linked source (10 §4.15).",
    "x-objectType": "mediaSequence",
    required: [
      "id",
      "type",
    ],
    if: {
      required: [
        "rev",
      ],
    },
    then: {
      required: [
        "id",
        "type",
        "rev",
        "assetId",
        "startFrame",
        "stopFrame",
        "linkOffset",
      ],
    },
    else: {
      propertyNames: {
        enum: [
          "id",
          "type",
          "name",
        ],
      },
    },
  },
  MotionPoint: {
    type: "object",
    additionalProperties: false,
    properties: {
      id: {
        $ref: "#/$defs/Id",
        description: "ID, unique within the channel (ID-03).",
        readOnly: true,
        "x-since": "1.0",
      },
      type: {
        type: "string",
        const: "motionPoint",
        readOnly: true,
        description: "Always `motionPoint`.",
        "x-since": "1.0",
      },
      rev: {
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
        description: "Revision of the last change.",
        readOnly: true,
        "x-since": "1.0",
      },
      frame: {
        type: [
          "integer",
          "null",
        ],
        maximum: 2147483647,
        minimum: 1,
        description: "Keyframe frame (integer, unique); `null` = control point.",
        "x-unit": "frame",
        "x-since": "1.0",
      },
      position: {
        $ref: "#/$defs/Point3",
        description: "Position in parent space (fields).",
        "x-unit": "field",
        "x-since": "1.0",
      },
      tension: {
        type: "number",
        maximum: 1,
        description: "TCB tension, −1…1.",
        minimum: -1,
        "x-since": "1.0",
      },
      continuity: {
        type: "number",
        maximum: 1,
        description: "TCB continuity, −1…1.",
        minimum: -1,
        "x-since": "1.0",
      },
      bias: {
        type: "number",
        maximum: 1,
        description: "TCB bias, −1…1.",
        minimum: -1,
        "x-since": "1.0",
      },
      constantSegment: {
        type: "boolean",
        description: "The segment starting at this keyframe holds the position.",
        "x-since": "1.0",
      },
      lockDepth: {
        type: "boolean",
        description: "Constant Z for the segment starting here (05 §9.3).",
        "x-since": "1.0",
      },
      meta: {
        type: "object",
        additionalProperties: true,
        description: "Free-form metadata.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      ext: {
        $ref: "#/$defs/Ext",
        description: "Extension data.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      path: {
        type: "string",
        description: "Canonical path (10 §2.4).",
        "x-transient": true,
        readOnly: true,
        "x-since": "1.0",
      },
    },
    "x-since": "1.0",
    description: "A point of a motion path (10 §4.12): a keyframe (`frame` set) or a control point (`frame: null`).",
    "x-objectType": "motionPoint",
    required: [
      "id",
      "type",
    ],
    if: {
      required: [
        "rev",
      ],
    },
    then: {
      required: [
        "id",
        "type",
        "rev",
        "frame",
        "position",
        "tension",
        "continuity",
        "bias",
        "constantSegment",
        "lockDepth",
      ],
    },
    else: {
      propertyNames: {
        enum: [
          "id",
          "type",
          "name",
        ],
      },
    },
  },
  MouthMapping: {
    type: "object",
    additionalProperties: false,
    properties: {
      A: {
        anyOf: [
          {
            $ref: "#/$defs/Id",
          },
          {
            type: "null",
          },
        ],
        description: "Drawing for A.",
        "x-since": "1.0",
      },
      B: {
        anyOf: [
          {
            $ref: "#/$defs/Id",
          },
          {
            type: "null",
          },
        ],
        description: "Drawing for B.",
        "x-since": "1.0",
      },
      C: {
        anyOf: [
          {
            $ref: "#/$defs/Id",
          },
          {
            type: "null",
          },
        ],
        description: "Drawing for C.",
        "x-since": "1.0",
      },
      D: {
        anyOf: [
          {
            $ref: "#/$defs/Id",
          },
          {
            type: "null",
          },
        ],
        description: "Drawing for D.",
        "x-since": "1.0",
      },
      E: {
        anyOf: [
          {
            $ref: "#/$defs/Id",
          },
          {
            type: "null",
          },
        ],
        description: "Drawing for E.",
        "x-since": "1.0",
      },
      F: {
        anyOf: [
          {
            $ref: "#/$defs/Id",
          },
          {
            type: "null",
          },
        ],
        description: "Drawing for F.",
        "x-since": "1.0",
      },
      G: {
        anyOf: [
          {
            $ref: "#/$defs/Id",
          },
          {
            type: "null",
          },
        ],
        description: "Drawing for G.",
        "x-since": "1.0",
      },
      X: {
        anyOf: [
          {
            $ref: "#/$defs/Id",
          },
          {
            type: "null",
          },
        ],
        description: "Drawing for X.",
        "x-since": "1.0",
      },
    },
    description: "Which drawing represents each mouth shape (07 §10.8).",
    required: [
      "A",
      "B",
      "C",
      "D",
      "E",
      "F",
      "G",
      "X",
    ],
  },
  MouthShape: {
    oneOf: [
      {
        type: "string",
        const: "A",
        description: "A.",
      },
      {
        type: "string",
        const: "B",
        description: "B.",
      },
      {
        type: "string",
        const: "C",
        description: "C.",
      },
      {
        type: "string",
        const: "D",
        description: "D.",
      },
      {
        type: "string",
        const: "E",
        description: "E.",
      },
      {
        type: "string",
        const: "F",
        description: "F.",
      },
      {
        type: "string",
        const: "G",
        description: "G.",
      },
      {
        type: "string",
        const: "X",
        description: "X (rest).",
      },
    ],
    description: "Mouth shape letters (07 §10; 10 §4.8 `mouthShape`).",
  },
  MouthTrack: {
    type: "object",
    additionalProperties: false,
    properties: {
      mouths: {
        type: "string",
        description: "One letter (`A`–`G`, `X`) per frame of the placement.",
        pattern: "^[A-GX]*$",
        "x-since": "1.0",
      },
      manual: {
        type: "array",
        items: {
          $ref: "#/$defs/FrameRange",
        },
        description: "Placement-relative frame ranges set by hand or explicitly.",
        "x-since": "1.0",
      },
      labels: {
        type: "object",
        additionalProperties: false,
        patternProperties: {
          "^(?:[1-9][0-9]{0,8}|1[0-9]{9}|20[0-9]{8}|21[0-3][0-9]{7}|214[0-6][0-9]{6}|2147[0-3][0-9]{5}|21474[0-7][0-9]{4}|214748[0-2][0-9]{3}|2147483[0-5][0-9]{2}|21474836[0-3][0-9]|214748364[0-7])$": {
            type: "string",
          },
        },
        description: "Optional display labels, keyed by placement-relative frame.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      source: {
        $ref: "#/$defs/MouthTrackSource",
        description: "How the track was produced.",
        "x-since": "1.0",
      },
      producedFor: {
        $ref: "#/$defs/ProducedFor",
        description: "Timing snapshot.",
        "x-since": "1.0",
      },
      stale: {
        type: "boolean",
        "x-derived": true,
        readOnly: true,
        "x-since": "1.0",
      },
    },
    required: [
      "mouths",
      "manual",
      "source",
      "producedFor",
    ],
    description: "A placement's lip-sync track (10 §4.14, 07 §10.3).",
  },
  MouthTrackSource: {
    oneOf: [
      {
        type: "string",
        const: "detected",
        description: "Detected from audio.",
      },
      {
        type: "string",
        const: "transcript",
        description: "Aligned to a transcript.",
      },
      {
        type: "string",
        const: "phonemes",
        description: "From explicit phonemes.",
      },
      {
        type: "string",
        const: "explicit",
        description: "Set explicitly.",
      },
    ],
    description: "How a mouth track was produced (10 §4.14).",
  },
  NodeKind: {
    oneOf: [
      {
        type: "string",
        const: "layer",
        description: "Planar stroke graph.",
      },
      {
        type: "string",
        const: "group",
        description: "Child nodes.",
      },
      {
        type: "string",
        const: "text",
        description: "Rich text object.",
      },
      {
        type: "string",
        const: "bitmap",
        description: "Embedded raster image.",
      },
    ],
    description: "Kind of a drawing node (02 §3.1).",
  },
  ObjectType: {
    type: "string",
    enum: [
      "project",
      "palette",
      "paletteStyle",
      "swatch",
      "pen",
      "asset",
      "drawingBank",
      "drawing",
      "drawingNode",
      "stroke",
      "scene",
      "transition",
      "element",
      "channel",
      "key",
      "motionPoint",
      "particleBlock",
      "soundPlacement",
      "mediaSequence",
      "library",
      "libraryFolder",
      "libraryItem",
    ],
  },
  Palette: {
    type: "object",
    additionalProperties: false,
    properties: {
      id: {
        $ref: "#/$defs/Id",
        description: "Immutable ID.",
        readOnly: true,
        "x-since": "1.0",
      },
      type: {
        type: "string",
        const: "palette",
        readOnly: true,
        description: "Always `palette`.",
        "x-since": "1.0",
      },
      rev: {
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
        description: "Revision of the last change.",
        readOnly: true,
        "x-since": "1.0",
      },
      name: {
        pattern: "^(?:[^\\u0000-\\u001F\\u007F\\\\/:*?\"<>|\\s]|[^\\u0000-\\u001F\\u007F\\\\/:*?\"<>|\\s][^\\u0000-\\u001F\\u007F\\\\/:*?\"<>|]*[^\\u0000-\\u001F\\u007F\\\\/:*?\"<>|\\s])$",
        description: "Unique in the project.",
        "x-command": "colour.palette.rename",
        minLength: 1,
        maxLength: 128,
        type: "string",
        "x-since": "1.0",
      },
      styles: {
        type: "array",
        items: {
          $ref: "#/$defs/PaletteStyle",
        },
        description: "Styles in order; at least one.",
        minItems: 1,
        "x-since": "1.0",
      },
      swatches: {
        type: "array",
        items: {
          $ref: "#/$defs/Swatch",
        },
        description: "Swatches in user order; may be empty.",
        "x-since": "1.0",
      },
      hidden: {
        type: "boolean",
        description: "Internal palette never shown to the user (03 §1.2; SI-005).",
        readOnly: true,
        "x-since": "1.0",
      },
      meta: {
        type: "object",
        additionalProperties: true,
        description: "Free-form metadata.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      ext: {
        $ref: "#/$defs/Ext",
        description: "Extension data.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      path: {
        type: "string",
        description: "Canonical path (10 §2.4).",
        "x-transient": true,
        readOnly: true,
        "x-since": "1.0",
      },
    },
    "x-since": "1.0",
    description: "A palette (10 §4.13, 03 §1.3 and §12.3).",
    "x-objectType": "palette",
    required: [
      "id",
      "type",
    ],
    if: {
      required: [
        "rev",
      ],
    },
    then: {
      required: [
        "id",
        "type",
        "rev",
        "name",
        "styles",
        "swatches",
        "hidden",
      ],
    },
    else: {
      propertyNames: {
        enum: [
          "id",
          "type",
          "name",
        ],
      },
    },
  },
  PaletteStyle: {
    type: "object",
    additionalProperties: false,
    properties: {
      id: {
        $ref: "#/$defs/Id",
        description: "Immutable ID.",
        readOnly: true,
        "x-since": "1.0",
      },
      type: {
        type: "string",
        const: "paletteStyle",
        readOnly: true,
        description: "Always `paletteStyle`.",
        "x-since": "1.0",
      },
      rev: {
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
        description: "Revision of the last change.",
        readOnly: true,
        "x-since": "1.0",
      },
      name: {
        pattern: "^(?:[^\\u0000-\\u001F\\u007F\\\\/:*?\"<>|\\s]|[^\\u0000-\\u001F\\u007F\\\\/:*?\"<>|\\s][^\\u0000-\\u001F\\u007F\\\\/:*?\"<>|]*[^\\u0000-\\u001F\\u007F\\\\/:*?\"<>|\\s])$",
        description: "Unique in the palette.",
        "x-command": "colour.style.rename",
        minLength: 1,
        maxLength: 128,
        type: "string",
        "x-since": "1.0",
      },
      meta: {
        type: "object",
        additionalProperties: true,
        description: "Free-form metadata.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      ext: {
        $ref: "#/$defs/Ext",
        description: "Extension data.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      path: {
        type: "string",
        description: "Canonical path (10 §2.4).",
        "x-transient": true,
        readOnly: true,
        "x-since": "1.0",
      },
    },
    "x-since": "1.0",
    description: "A palette style: an alternative set of values for all swatches of its palette (03 §1.5).",
    "x-objectType": "paletteStyle",
    required: [
      "id",
      "type",
    ],
    if: {
      required: [
        "rev",
      ],
    },
    then: {
      required: [
        "id",
        "type",
        "rev",
        "name",
      ],
    },
    else: {
      propertyNames: {
        enum: [
          "id",
          "type",
          "name",
        ],
      },
    },
  },
  Paragraph: {
    type: "object",
    additionalProperties: false,
    properties: {
      start: {
        type: "integer",
        maximum: 2147483647,
        minimum: 0,
        description: "First code point.",
        "x-since": "1.0",
      },
      length: {
        type: "integer",
        maximum: 2147483647,
        minimum: 0,
        description: "Number of code points.",
        "x-since": "1.0",
      },
      alignment: {
        $ref: "#/$defs/Alignment",
        description: "Alignment.",
        "x-since": "1.0",
      },
      marginLeft: {
        type: "number",
        "x-unit": "du",
        description: "Left margin (du).",
        "x-since": "1.0",
      },
      marginRight: {
        type: "number",
        "x-unit": "du",
        description: "Right margin (du).",
        "x-since": "1.0",
      },
      indent: {
        type: "number",
        "x-unit": "du",
        description: "First-line indent (du).",
        "x-since": "1.0",
      },
      lineSpacing: {
        type: "number",
        description: "Line spacing factor.",
        "x-since": "1.0",
      },
    },
    required: [
      "start",
      "length",
      "alignment",
      "marginLeft",
      "marginRight",
      "indent",
      "lineSpacing",
    ],
    description: "Paragraph formatting of a text node (10 §4.9).",
  },
  ParamSlot: {
    oneOf: [
      {
        type: "string",
        const: "mask",
        description: "Clipping mask.",
      },
      {
        type: "string",
        const: "sprites",
        description: "Particle sprites.",
      },
      {
        type: "string",
        const: "bones",
        description: "Skeleton bones.",
      },
      {
        type: "string",
        const: "skin",
        description: "Skeleton skin.",
      },
    ],
    description: "Slot of an effect-parameter element (06 §2.3).",
  },
  ParentColourTransformMode: {
    oneOf: [
      {
        type: "string",
        const: "drawingsOnly",
        description: "To the original drawings only (default).",
      },
      {
        type: "string",
        const: "drawingsAndShadow",
        description: "To drawings and shadow.",
      },
      {
        type: "string",
        const: "shadowOnly",
        description: "To the shadow only.",
      },
    ],
    description: "How an inherited Colour Transform applies to a Drop Shadow (06 §3.4).",
  },
  ParticleBackground: {
    oneOf: [
      {
        type: "string",
        const: "transparent",
        description: "None (default).",
      },
      {
        type: "string",
        const: "darkSkyGradient",
        description: "Dark blue sky gradient.",
      },
      {
        type: "string",
        const: "lightSkyGradient",
        description: "Light blue sky gradient.",
      },
    ],
    description: "Background of a particle block (06 §3.8.3).",
  },
  ParticleBlock: {
    type: "object",
    additionalProperties: false,
    properties: {
      id: {
        $ref: "#/$defs/Id",
        description: "Immutable ID.",
        readOnly: true,
        "x-since": "1.0",
      },
      type: {
        type: "string",
        const: "particleBlock",
        readOnly: true,
        description: "Always `particleBlock`.",
        "x-since": "1.0",
      },
      rev: {
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
        description: "Revision of the last change.",
        readOnly: true,
        "x-since": "1.0",
      },
      particleType: {
        $ref: "#/$defs/ParticleType",
        description: "Particle type (06 Block `type`; SI-004).",
        "x-since": "1.0",
      },
      variant: {
        type: [
          "string",
          "null",
        ],
        description: "Sub-behaviour; `null` = type default.",
        "x-since": "1.0",
      },
      presetId: {
        type: [
          "string",
          "null",
        ],
        description: "Last applied preset (informational).",
        "x-since": "1.0",
      },
      seed: {
        type: "integer",
        maximum: 4294967295,
        minimum: 0,
        description: "Seed of all random choices, 0–2³²−1 (06 §3.8.3, §4.6: a full unsigned 32-bit range, stated explicitly).",
        "x-since": "1.0",
      },
      spriteSetId: {
        type: [
          "string",
          "null",
        ],
        description: "Built-in sprite set.",
        "x-since": "1.0",
      },
      static: {
        $ref: "#/$defs/BlockStatic",
        description: "Static parameters.",
        "x-since": "1.0",
      },
      dynamic: {
        type: "object",
        additionalProperties: {
          $ref: "#/$defs/DynamicParam",
        },
        description: "Dynamic parameters by id (exactly the ids of 06 §3.8.4).",
        "x-since": "1.0",
      },
      meta: {
        type: "object",
        additionalProperties: true,
        description: "Free-form metadata.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      ext: {
        $ref: "#/$defs/Ext",
        description: "Extension data.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      path: {
        type: "string",
        description: "Canonical path (10 §2.4).",
        "x-transient": true,
        readOnly: true,
        "x-since": "1.0",
      },
      index: {
        type: "integer",
        minimum: 0,
        maximum: 2147483647,
        "x-derived": true,
        readOnly: true,
        "x-since": "1.0",
      },
    },
    "x-since": "1.0",
    description: "A simulation block of a particle effect (10 §4.6, 06 §3.8.2, §8.4).",
    "x-objectType": "particleBlock",
    required: [
      "id",
      "type",
    ],
    if: {
      required: [
        "rev",
      ],
    },
    then: {
      required: [
        "id",
        "type",
        "rev",
        "particleType",
        "variant",
        "presetId",
        "seed",
        "spriteSetId",
        "static",
        "dynamic",
      ],
    },
    else: {
      propertyNames: {
        enum: [
          "id",
          "type",
          "name",
        ],
      },
    },
  },
  ParticleParams: {
    type: "object",
    additionalProperties: false,
    description: "Particle effects have no effect-level parameters; their content is in `blocks` (06 §3.8).",
  },
  ParticleType: {
    oneOf: [
      {
        type: "string",
        const: "rain",
        description: "Nature: rain.",
      },
      {
        type: "string",
        const: "snow",
        description: "Nature: snow.",
      },
      {
        type: "string",
        const: "hail",
        description: "Nature: hail.",
      },
      {
        type: "string",
        const: "fog",
        description: "Nature: fog.",
      },
      {
        type: "string",
        const: "cloud",
        description: "Nature: cloud.",
      },
      {
        type: "string",
        const: "eruption",
        description: "Nature: eruption.",
      },
      {
        type: "string",
        const: "shatter",
        description: "Action: shatter.",
      },
      {
        type: "string",
        const: "magic",
        description: "Action: magic.",
      },
      {
        type: "string",
        const: "explosion",
        description: "Action: explosion.",
      },
      {
        type: "string",
        const: "splash",
        description: "Action: splash.",
      },
      {
        type: "string",
        const: "fire",
        description: "Action: fire.",
      },
      {
        type: "string",
        const: "celestial",
        description: "Action: celestial.",
      },
    ],
    description: "Particle types (06 §3.8.1). Persisted as `particleType` because `type` is the envelope field (SI-004).",
  },
  Pen: {
    type: "object",
    additionalProperties: false,
    properties: {
      id: {
        $ref: "#/$defs/Id",
        description: "Immutable ID.",
        readOnly: true,
        "x-since": "1.0",
      },
      type: {
        type: "string",
        const: "pen",
        readOnly: true,
        description: "Always `pen`.",
        "x-since": "1.0",
      },
      rev: {
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
        description: "Revision of the last change (10 REV-02).",
        readOnly: true,
        "x-since": "1.0",
      },
      name: {
        pattern: "^(?:[^\\u0000-\\u001F\\u007F\\\\/:*?\"<>|\\s]|[^\\u0000-\\u001F\\u007F\\\\/:*?\"<>|\\s][^\\u0000-\\u001F\\u007F\\\\/:*?\"<>|]*[^\\u0000-\\u001F\\u007F\\\\/:*?\"<>|\\s])$",
        description: "Unique in the project; default `Pen <n>`.",
        "x-command": "tool.pen.rename",
        minLength: 1,
        maxLength: 64,
        type: "string",
        "x-since": "1.0",
      },
      kind: {
        $ref: "#/$defs/PenKind",
        description: "Brush or eraser pen (02 §6.1; SI-004).",
        "x-since": "1.0",
      },
      minSize: {
        type: "number",
        "x-unit": "du",
        description: "Width at lowest pressure, 0–100 du; `minSize ≤ maxSize`.",
        minimum: 0,
        maximum: 100,
        "x-since": "1.0",
      },
      maxSize: {
        type: "number",
        "x-unit": "du",
        description: "Width at full pressure, 0–100 du.",
        minimum: 0,
        maximum: 100,
        "x-since": "1.0",
      },
      smoothness: {
        type: "integer",
        description: "Curve-fitting tolerance and pressure smoothing, 0–10 (10 §4.2: int).",
        minimum: 0,
        maximum: 10,
        "x-since": "1.0",
      },
      tip: {
        $ref: "#/$defs/Tip",
        description: "Tip shape.",
        "x-since": "1.0",
      },
      tipAngle: {
        type: "number",
        "x-unit": "degree",
        description: "Orientation of non-round tips, 0–360 degrees (02 §6.1; SI-004).",
        minimum: 0,
        maximum: 360,
        "x-since": "1.0",
      },
      colourId: {
        anyOf: [
          {
            $ref: "#/$defs/Id",
          },
          {
            type: "null",
          },
        ],
        description: "Colour bound to the pen; `null` = use the current colour.",
        "x-ref": "swatch",
        "x-since": "1.0",
      },
      meta: {
        type: "object",
        additionalProperties: true,
        description: "Free-form metadata.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      ext: {
        $ref: "#/$defs/Ext",
        description: "Extension data.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      path: {
        type: "string",
        description: "Canonical path (10 §2.4).",
        "x-transient": true,
        readOnly: true,
        "x-since": "1.0",
      },
    },
    "x-since": "1.0",
    description: "A pen preset (10 §4.2). Unique name in the project.",
    "x-objectType": "pen",
    required: [
      "id",
      "type",
    ],
    if: {
      required: [
        "rev",
      ],
    },
    then: {
      required: [
        "id",
        "type",
        "rev",
        "name",
        "kind",
        "minSize",
        "maxSize",
        "smoothness",
        "tip",
        "tipAngle",
        "colourId",
      ],
    },
    else: {
      propertyNames: {
        enum: [
          "id",
          "type",
          "name",
        ],
      },
    },
  },
  PenKind: {
    oneOf: [
      {
        type: "string",
        const: "brush",
        description: "Used by the Brush (and Pencil/shape thickness).",
      },
      {
        type: "string",
        const: "eraser",
        description: "Used by the Eraser.",
      },
    ],
    description: "Tool family of a pen (02 §6.1 \"Type\"). Persisted as `kind` because `type` is the envelope field (SI-004).",
  },
  Point2: {
    type: "object",
    additionalProperties: false,
    properties: {
      x: {
        type: "number",
        description: "X coordinate.",
        "x-since": "1.0",
      },
      y: {
        type: "number",
        description: "Y coordinate.",
        "x-since": "1.0",
      },
    },
    required: [
      "x",
      "y",
    ],
    description: "A 2-D point `{x, y}` (10 §1.1).",
  },
  Point3: {
    type: "object",
    additionalProperties: false,
    properties: {
      x: {
        type: "number",
        description: "X coordinate.",
        "x-since": "1.0",
      },
      y: {
        type: "number",
        description: "Y coordinate.",
        "x-since": "1.0",
      },
      z: {
        type: "number",
        description: "Z coordinate.",
        "x-since": "1.0",
      },
    },
    required: [
      "x",
      "y",
      "z",
    ],
    description: "A 3-D point `{x, y, z}` (10 §1.1).",
  },
  PositionLoop: {
    oneOf: [
      {
        type: "string",
        const: "cycle",
        description: "Every cycle restarts from the first position.",
      },
      {
        type: "string",
        const: "offset",
        description: "Cycles advance by the first cycle's displacement.",
      },
    ],
    description: "Position looping mode (05 §8.4).",
  },
  ProducedFor: {
    type: "object",
    additionalProperties: false,
    properties: {
      assetId: {
        $ref: "#/$defs/Id",
        description: "Asset.",
        "x-ref": "asset",
        "x-since": "1.0",
      },
      inPoint: {
        type: "number",
        "x-unit": "second",
        description: "In-point (seconds).",
        "x-since": "1.0",
      },
      outPoint: {
        type: "number",
        "x-unit": "second",
        description: "Out-point (seconds).",
        "x-since": "1.0",
      },
      loops: {
        type: "integer",
        maximum: 2147483647,
        minimum: 0,
        description: "Loop count.",
        "x-since": "1.0",
      },
      stopFrame: {
        type: "integer",
        maximum: 2147483647,
        minimum: 0,
        description: "Stop frame.",
        "x-unit": "frame",
        "x-since": "1.0",
      },
      frameRate: {
        type: "number",
        description: "Project frame rate.",
        "x-since": "1.0",
      },
    },
    required: [
      "assetId",
      "inPoint",
      "outPoint",
      "loops",
      "stopFrame",
      "frameRate",
    ],
    description: "The timing a mouth track was produced for (10 §4.14).",
  },
  Project: {
    type: "object",
    additionalProperties: false,
    properties: {
      id: {
        $ref: "#/$defs/Id",
        description: "Immutable ID.",
        readOnly: true,
        "x-since": "1.0",
      },
      type: {
        type: "string",
        const: "project",
        readOnly: true,
        description: "Always `project`.",
        "x-since": "1.0",
      },
      rev: {
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
        description: "Revision of the last change.",
        readOnly: true,
        "x-since": "1.0",
      },
      name: {
        pattern: "^(?:[^\\u0000-\\u001F\\u007F\\\\/:*?\"<>|\\s]|[^\\u0000-\\u001F\\u007F\\\\/:*?\"<>|\\s][^\\u0000-\\u001F\\u007F\\\\/:*?\"<>|]*[^\\u0000-\\u001F\\u007F\\\\/:*?\"<>|\\s.])$",
        description: "Project name; default `Untitled`; also the default package file stem.",
        minLength: 1,
        maxLength: 100,
        type: "string",
        "x-since": "1.0",
      },
      createdAt: {
        $ref: "#/$defs/Timestamp",
        description: "Creation time.",
        readOnly: true,
        "x-since": "1.0",
      },
      modifiedAt: {
        $ref: "#/$defs/Timestamp",
        description: "Set by each save.",
        readOnly: true,
        "x-since": "1.0",
      },
      frameRate: {
        type: "number",
        "x-command": "project.properties.set",
        description: "Frames per second, 1–120; NTSC rates are exact as written.",
        minimum: 1,
        maximum: 120,
        "x-since": "1.0",
      },
      cameraWidth: {
        type: "integer",
        "x-command": "project.properties.set",
        minimum: 16,
        description: "Camera (output) width, 16–8192 pixels.",
        maximum: 8192,
        "x-unit": "pixel",
        "x-since": "1.0",
      },
      cameraHeight: {
        type: "integer",
        "x-command": "project.properties.set",
        minimum: 16,
        description: "Camera (output) height, 16–8192 pixels.",
        maximum: 8192,
        "x-unit": "pixel",
        "x-since": "1.0",
      },
      autoColourUpdate: {
        type: "boolean",
        description: "Whether template swatch values overwrite project values with the same colour IDs (03 §1.2; SI-005).",
        "x-since": "1.0",
      },
      startTransition: {
        $ref: "#/$defs/Transition",
        description: "Edit point before the first scene.",
        "x-since": "1.0",
      },
      palettes: {
        type: "array",
        items: {
          $ref: "#/$defs/Palette",
        },
        "x-childList": "palette",
        description: "Palettes in display order; at least one.",
        minItems: 1,
        "x-since": "1.0",
      },
      pens: {
        type: "array",
        items: {
          $ref: "#/$defs/Pen",
        },
        description: "Pens; at least one.",
        minItems: 1,
        "x-since": "1.0",
      },
      assets: {
        type: "array",
        items: {
          $ref: "#/$defs/Asset",
        },
        description: "Media assets.",
        "x-since": "1.0",
      },
      drawingBanks: {
        type: "array",
        items: {
          $ref: "#/$defs/DrawingBank",
        },
        "x-childList": "drawingBank",
        description: "Drawing banks (unordered; written in ID order).",
        "x-since": "1.0",
      },
      scenes: {
        type: "array",
        items: {
          $ref: "#/$defs/Scene",
        },
        "x-childList": "scene",
        description: "Scenes in storyline order; at least one.",
        minItems: 1,
        "x-since": "1.0",
      },
      library: {
        allOf: [
          {
            $ref: "#/$defs/Library",
          },
        ],
        "x-childRef": "library",
        description: "The project-local library.",
        "x-since": "1.0",
      },
      meta: {
        type: "object",
        additionalProperties: true,
        description: "Free-form metadata.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      ext: {
        $ref: "#/$defs/Ext",
        description: "Extension data.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      path: {
        type: "string",
        description: "Canonical path (10 §2.4).",
        "x-transient": true,
        readOnly: true,
        "x-since": "1.0",
      },
      sceneStartFrames: {
        type: "array",
        items: {
          type: "integer",
          minimum: 0,
          maximum: 2147483647,
        },
        "x-derived": true,
        readOnly: true,
        "x-since": "1.0",
      },
      globalLength: {
        type: "integer",
        minimum: 0,
        maximum: 2147483647,
        "x-derived": true,
        readOnly: true,
        "x-since": "1.0",
      },
      thumbnail: {
        anyOf: [
          {
            $ref: "#/$defs/Blob",
          },
          {
            type: "null",
          },
        ],
        "x-derived": true,
        readOnly: true,
        "x-since": "1.0",
      },
    },
    "x-since": "1.0",
    description: "The project (10 §4.1). Stored in `project.json` with stubs of palettes, drawing banks, scenes and the local\nlibrary.",
    "x-objectType": "project",
    required: [
      "id",
      "type",
    ],
    if: {
      required: [
        "rev",
      ],
    },
    then: {
      required: [
        "id",
        "type",
        "rev",
        "name",
        "createdAt",
        "modifiedAt",
        "frameRate",
        "cameraWidth",
        "cameraHeight",
        "autoColourUpdate",
        "startTransition",
        "palettes",
        "pens",
        "assets",
        "drawingBanks",
        "scenes",
        "library",
      ],
    },
    else: {
      propertyNames: {
        enum: [
          "id",
          "type",
          "name",
        ],
      },
    },
  },
  RecoveryStyle: {
    type: "object",
    additionalProperties: false,
    properties: {
      styleId: {
        $ref: "#/$defs/Id",
        description: "Style ID.",
        "x-since": "1.0",
      },
      styleName: {
        type: "string",
        description: "Style name.",
        "x-since": "1.0",
      },
    },
    required: [
      "styleId",
      "styleName",
    ],
    description: "A style reference inside a colour-recovery entry.",
  },
  Role: {
    oneOf: [
      {
        type: "string",
        const: "normal",
        description: "Normal compositing.",
      },
      {
        type: "string",
        const: "foreground",
        description: "In front of all normal elements.",
      },
      {
        type: "string",
        const: "background",
        description: "Behind all normal elements; auto-extends to the scene length.",
      },
      {
        type: "string",
        const: "envelope",
        description: "Marks a bone-deformation skin.",
      },
    ],
    description: "Composite role (04 §6.1, 05 §8.1; SI-003: the superset of 04/05 and 10 §4.5).",
  },
  Scale2: {
    type: "object",
    additionalProperties: false,
    properties: {
      x: {
        type: "number",
        not: {
          exclusiveMinimum: -0.001,
          exclusiveMaximum: 0.001,
        },
        description: "Horizontal factor.",
        "x-since": "1.0",
      },
      y: {
        type: "number",
        not: {
          exclusiveMinimum: -0.001,
          exclusiveMaximum: 0.001,
        },
        description: "Vertical factor.",
        "x-since": "1.0",
      },
    },
    required: [
      "x",
      "y",
    ],
    description: "A 2-D scale `{x, y}`; `|s| ≥ 0.001`, negative values flip.",
  },
  Scene: {
    type: "object",
    additionalProperties: false,
    properties: {
      id: {
        $ref: "#/$defs/Id",
        description: "Immutable ID.",
        readOnly: true,
        "x-since": "1.0",
      },
      type: {
        type: "string",
        const: "scene",
        readOnly: true,
        description: "Always `scene`.",
        "x-since": "1.0",
      },
      rev: {
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
        description: "Revision of the last change.",
        readOnly: true,
        "x-since": "1.0",
      },
      name: {
        pattern: "^(?:[^\\u0000-\\u001F\\u007F\\\\/:*?\"<>|\\s]|[^\\u0000-\\u001F\\u007F\\\\/:*?\"<>|\\s][^\\u0000-\\u001F\\u007F\\\\/:*?\"<>|]*[^\\u0000-\\u001F\\u007F\\\\/:*?\"<>|\\s])$",
        description: "Unique in the project; default `Scene-<n>`.",
        "x-command": "storyline.scene.rename",
        minLength: 1,
        maxLength: 128,
        type: "string",
        "x-since": "1.0",
      },
      visible: {
        type: "boolean",
        description: "Hidden scenes are skipped by storyline playback and export.",
        "x-command": "storyline.scene.visible.set",
        "x-since": "1.0",
      },
      backgroundColour: {
        $ref: "#/$defs/Colour",
        description: "Background colour (default `#FFFFFFFF`).",
        "x-command": "storyline.scene.set",
        "x-since": "1.0",
      },
      currentCameraId: {
        anyOf: [
          {
            $ref: "#/$defs/Id",
          },
          {
            type: "null",
          },
        ],
        description: "The current camera; must be a camera of this scene when it has one.",
        "x-ref": "element",
        "x-command": "anim.camera.current.set",
        "x-since": "1.0",
      },
      paletteStyles: {
        type: "object",
        additionalProperties: false,
        patternProperties: {
          "^[A-Za-z0-9_-]{1,64}$": {
            $ref: "#/$defs/Id",
          },
        },
        description: "Active style per palette: exactly one entry per palette, in palette order.",
        "x-command": "colour.style.select",
        "x-since": "1.0",
      },
      outTransition: {
        $ref: "#/$defs/Transition",
        description: "Edit point after this scene.",
        "x-since": "1.0",
      },
      tree: {
        type: "array",
        items: {
          $ref: "#/$defs/TreeNode",
        },
        description: "Timeline order and hierarchy; every element exactly once.",
        "x-since": "1.0",
      },
      columnOrder: {
        type: "array",
        items: {
          $ref: "#/$defs/Id",
        },
        description: "Exposure Sheet order of the cell-based elements, back to front.",
        "x-since": "1.0",
      },
      elements: {
        type: "array",
        items: {
          $ref: "#/$defs/Element",
        },
        "x-childList": "element",
        description: "The elements (unordered; written in ID order).",
        "x-since": "1.0",
      },
      meta: {
        type: "object",
        additionalProperties: true,
        description: "Free-form metadata.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      ext: {
        $ref: "#/$defs/Ext",
        description: "Extension data.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      path: {
        type: "string",
        description: "Canonical path (10 §2.4).",
        "x-transient": true,
        readOnly: true,
        "x-since": "1.0",
      },
      length: {
        type: "integer",
        minimum: 1,
        maximum: 100000,
        "x-derived": true,
        readOnly: true,
        "x-since": "1.0",
      },
      paddedLength: {
        type: "integer",
        minimum: 0,
        maximum: 2147483647,
        "x-derived": true,
        readOnly: true,
        "x-since": "1.0",
      },
    },
    "x-since": "1.0",
    description: "A scene (10 §4.3). Stored in `scenes/<sceneId>/scene.json` with element stubs.",
    "x-objectType": "scene",
    required: [
      "id",
      "type",
    ],
    if: {
      required: [
        "rev",
      ],
    },
    then: {
      required: [
        "id",
        "type",
        "rev",
        "name",
        "visible",
        "backgroundColour",
        "currentCameraId",
        "paletteStyles",
        "outTransition",
        "tree",
        "columnOrder",
        "elements",
      ],
    },
    else: {
      propertyNames: {
        enum: [
          "id",
          "type",
          "name",
        ],
      },
    },
  },
  Script: {
    oneOf: [
      {
        type: "string",
        const: "normal",
        description: "Normal.",
      },
      {
        type: "string",
        const: "subscript",
        description: "Subscript.",
      },
      {
        type: "string",
        const: "superscript",
        description: "Superscript.",
      },
    ],
    description: "Vertical script position of a text run.",
  },
  ShakeAxis: {
    oneOf: [
      {
        type: "string",
        const: "x",
        description: "East–west.",
      },
      {
        type: "string",
        const: "y",
        description: "North–south.",
      },
      {
        type: "string",
        const: "xy",
        description: "Both in the plane (default).",
      },
      {
        type: "string",
        const: "z",
        description: "Front–back.",
      },
      {
        type: "string",
        const: "xyz",
        description: "All three.",
      },
    ],
    description: "Shake axes (06 §3.6).",
  },
  ShakeParams: {
    type: "object",
    additionalProperties: false,
    properties: {
      axis: {
        $ref: "#/$defs/ShakeAxis",
        description: "Displacement axes (static).",
        "x-since": "1.0",
      },
      waveTime: {
        type: "integer",
        "x-unit": "frame",
        minimum: 1,
        description: "Oscillation period 1–999 frames (static).",
        maximum: 999,
        "x-since": "1.0",
      },
      decayHalfLifeFrames: {
        type: "number",
        "x-unit": "frame",
        description: "Half-life 0.1–999 frames (static).",
        minimum: 0.1,
        maximum: 999,
        "x-since": "1.0",
      },
      amplitude: {
        type: "number",
        "x-unit": "field",
        description: "Peak displacement 0–100 fields (animatable).",
        minimum: 0,
        maximum: 100,
        "x-since": "1.0",
      },
    },
    required: [
      "axis",
      "waveTime",
      "decayHalfLifeFrames",
      "amplitude",
    ],
    description: "Shake parameters (06 §3.6).",
  },
  Sides: {
    type: "object",
    additionalProperties: false,
    properties: {
      left: {
        anyOf: [
          {
            $ref: "#/$defs/ZoneFill",
          },
          {
            type: "null",
          },
        ],
        description: "Left side.",
        "x-since": "1.0",
      },
      right: {
        anyOf: [
          {
            $ref: "#/$defs/ZoneFill",
          },
          {
            type: "null",
          },
        ],
        description: "Right side.",
        "x-since": "1.0",
      },
    },
    description: "Fills on the left and right side of a stroke, relative to point order.",
    required: [
      "left",
      "right",
    ],
  },
  SkeletonBlock: {
    type: "object",
    additionalProperties: false,
    properties: {
      boneColour: {
        anyOf: [
          {
            $ref: "#/$defs/Colour",
          },
          {
            type: "null",
          },
        ],
        description: "Bone colour; `null` = preference.",
        "x-since": "1.0",
      },
      showTrack: {
        type: "boolean",
        description: "Show the track.",
        "x-since": "1.0",
      },
      showRoot: {
        type: "boolean",
        description: "Show the root.",
        "x-since": "1.0",
      },
      showBones: {
        type: "boolean",
        description: "Show bones.",
        "x-since": "1.0",
      },
      showInfluence: {
        type: "boolean",
        description: "Show influence.",
        "x-since": "1.0",
      },
      vectorQuality: {
        $ref: "#/$defs/VectorQuality",
        description: "Deformation quality.",
        "x-since": "1.0",
      },
      deformationEnabled: {
        type: "boolean",
        description: "Deformation on.",
        "x-since": "1.0",
      },
    },
    required: [
      "boneColour",
      "showTrack",
      "showRoot",
      "showBones",
      "showInfluence",
      "vectorQuality",
      "deformationEnabled",
    ],
    description: "Skeleton block (05 §13.6).",
  },
  SolidValue: {
    type: "object",
    additionalProperties: false,
    properties: {
      rgba: {
        $ref: "#/$defs/Colour",
        description: "The colour.",
        "x-since": "1.0",
      },
    },
    required: [
      "rgba",
    ],
    description: "`{rgba}`.",
  },
  SoundPlacement: {
    type: "object",
    additionalProperties: false,
    properties: {
      id: {
        $ref: "#/$defs/Id",
        description: "Immutable ID.",
        readOnly: true,
        "x-since": "1.0",
      },
      type: {
        type: "string",
        const: "soundPlacement",
        readOnly: true,
        description: "Always `soundPlacement`.",
        "x-since": "1.0",
      },
      rev: {
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
        description: "Revision of the last change.",
        readOnly: true,
        "x-since": "1.0",
      },
      assetId: {
        $ref: "#/$defs/Id",
        description: "A `sound` asset, or a `movie` asset with audio.",
        "x-ref": "asset",
        "x-since": "1.0",
      },
      startFrame: {
        type: "integer",
        maximum: 2147483647,
        minimum: 1,
        description: "First frame.",
        "x-unit": "frame",
        "x-since": "1.0",
      },
      stopFrame: {
        type: "integer",
        maximum: 2147483647,
        minimum: 1,
        description: "Last frame; `startFrame ≤ stopFrame ≤` natural end.",
        "x-unit": "frame",
        "x-since": "1.0",
      },
      inPoint: {
        type: "number",
        "x-unit": "second",
        description: "In-point in the source (seconds).",
        minimum: 0,
        "x-since": "1.0",
      },
      outPoint: {
        type: "number",
        "x-unit": "second",
        description: "Out-point in the source (seconds); `inPoint < outPoint`.",
        minimum: 0,
        "x-since": "1.0",
      },
      loops: {
        type: "integer",
        maximum: 2147483647,
        minimum: 1,
        description: "Repetitions of the trimmed segment.",
        "x-since": "1.0",
      },
      volume: {
        type: "integer",
        maximum: 50,
        description: "Global gain −50…+50.",
        minimum: -50,
        "x-since": "1.0",
      },
      envelope: {
        type: "array",
        items: {
          $ref: "#/$defs/EnvelopePoint",
        },
        description: "Gain points, times ascending.",
        "x-since": "1.0",
      },
      mouthTrack: {
        anyOf: [
          {
            $ref: "#/$defs/MouthTrack",
          },
          {
            type: "null",
          },
        ],
        description: "The placement's lip-sync track.",
        "x-since": "1.0",
      },
      meta: {
        type: "object",
        additionalProperties: true,
        description: "Free-form metadata.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      ext: {
        $ref: "#/$defs/Ext",
        description: "Extension data.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      path: {
        type: "string",
        description: "Canonical path (10 §2.4).",
        "x-transient": true,
        readOnly: true,
        "x-since": "1.0",
      },
      naturalEndFrame: {
        type: "integer",
        minimum: 0,
        maximum: 2147483647,
        "x-derived": true,
        readOnly: true,
        "x-since": "1.0",
      },
    },
    "x-since": "1.0",
    description: "One use of a sound asset in a sound element (10 §4.14, 07 §1.2).",
    "x-objectType": "soundPlacement",
    required: [
      "id",
      "type",
    ],
    if: {
      required: [
        "rev",
      ],
    },
    then: {
      required: [
        "id",
        "type",
        "rev",
        "assetId",
        "startFrame",
        "stopFrame",
        "inPoint",
        "outPoint",
        "loops",
        "volume",
        "envelope",
        "mouthTrack",
      ],
    },
    else: {
      propertyNames: {
        enum: [
          "id",
          "type",
          "name",
        ],
      },
    },
  },
  Spine: {
    type: "object",
    additionalProperties: false,
    properties: {
      points: {
        type: "array",
        items: {
          type: "array",
          items: {
            type: "number",
          },
          minItems: 6,
          maxItems: 6,
        },
        description: "The fitted centre path.",
        "x-unit": "du",
        "x-since": "1.0",
      },
      pressure: {
        type: "array",
        items: {
          type: "array",
          items: {
            type: "number",
          },
          minItems: 2,
          maxItems: 2,
        },
        description: "Pressure map: `(t, p)` samples ordered by `t`.",
        "x-since": "1.0",
      },
      pen: {
        $ref: "#/$defs/SpinePen",
        description: "Pen used.",
        "x-since": "1.0",
      },
    },
    required: [
      "points",
      "pressure",
      "pen",
    ],
    description: "The spine of an unedited brush stroke (02 §3.2).",
  },
  SpinePen: {
    type: "object",
    additionalProperties: false,
    properties: {
      penId: {
        anyOf: [
          {
            $ref: "#/$defs/Id",
          },
          {
            type: "null",
          },
        ],
        description: "The pen used, if it still exists.",
        "x-ref": "pen",
        "x-since": "1.0",
      },
      tip: {
        $ref: "#/$defs/Tip",
        description: "Tip shape.",
        "x-since": "1.0",
      },
      minSize: {
        type: "number",
        "x-unit": "du",
        description: "Minimum size (du).",
        minimum: 0,
        maximum: 100,
        "x-since": "1.0",
      },
      maxSize: {
        type: "number",
        "x-unit": "du",
        description: "Maximum size (du).",
        minimum: 0,
        maximum: 100,
        "x-since": "1.0",
      },
    },
    required: [
      "penId",
      "tip",
      "minSize",
      "maxSize",
    ],
    description: "The pen recorded on a brush spine (02 §3.2).",
  },
  Stroke: {
    type: "object",
    additionalProperties: false,
    properties: {
      id: {
        $ref: "#/$defs/Id",
        description: "ID, unique within the drawing (ID-03).",
        readOnly: true,
        "x-since": "1.0",
      },
      type: {
        type: "string",
        const: "stroke",
        readOnly: true,
        description: "Always `stroke`.",
        "x-since": "1.0",
      },
      rev: {
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
        description: "Revision of the last change.",
        readOnly: true,
        "x-since": "1.0",
      },
      kind: {
        $ref: "#/$defs/StrokeKind",
        description: "Brush, pencil or invisible.",
        "x-since": "1.0",
      },
      closed: {
        type: "boolean",
        description: "Whether the path is closed (brush outlines are).",
        "x-since": "1.0",
      },
      points: {
        type: "array",
        items: {
          type: "array",
          items: {
            type: "number",
          },
          minItems: 6,
          maxItems: 6,
        },
        description: "Anchors with handle offsets; at least 2 (1 for a closed dab).",
        minItems: 1,
        "x-unit": "du",
        "x-since": "1.0",
      },
      colourId: {
        anyOf: [
          {
            $ref: "#/$defs/Id",
          },
          {
            type: "null",
          },
        ],
        description: "Brush: fill colour; pencil: line colour; `null` for invisible strokes.",
        "x-ref": "swatch",
        "x-since": "1.0",
      },
      fillMatrix: {
        type: [
          "array",
          "null",
        ],
        items: {
          type: "number",
        },
        minItems: 6,
        maxItems: 6,
        description: "Placement of a gradient/texture colour space for brush strokes.",
        "x-since": "1.0",
      },
      lineStyle: {
        anyOf: [
          {
            $ref: "#/$defs/LineStyle",
          },
          {
            type: "null",
          },
        ],
        description: "Pencil strokes only.",
        "x-since": "1.0",
      },
      spine: {
        anyOf: [
          {
            $ref: "#/$defs/Spine",
          },
          {
            type: "null",
          },
        ],
        description: "Brush strokes while unedited.",
        "x-since": "1.0",
      },
      sides: {
        $ref: "#/$defs/Sides",
        description: "Zone fills on each side.",
        "x-since": "1.0",
      },
      meta: {
        type: "object",
        additionalProperties: true,
        description: "Free-form metadata.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      ext: {
        $ref: "#/$defs/Ext",
        description: "Extension data.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      path: {
        type: "string",
        description: "Canonical path (10 §2.4).",
        "x-transient": true,
        readOnly: true,
        "x-since": "1.0",
      },
    },
    "x-since": "1.0",
    description: "A stroke of a layer (10 §4.9, 02 §3.2). `lineStyle` is non-null for pencil strokes only; `spine` only for\nunedited brush strokes.",
    "x-objectType": "stroke",
    required: [
      "id",
      "type",
    ],
    if: {
      required: [
        "rev",
      ],
    },
    then: {
      required: [
        "id",
        "type",
        "rev",
        "kind",
        "closed",
        "points",
        "colourId",
        "fillMatrix",
        "lineStyle",
        "spine",
        "sides",
      ],
      allOf: [
        {
          if: {
            properties: {
              kind: {
                const: "pencil",
              },
            },
            required: [
              "kind",
            ],
          },
          then: {
            properties: {
              lineStyle: {
                $ref: "#/$defs/LineStyle",
              },
              spine: {
                type: "null",
              },
              fillMatrix: {
                type: "null",
              },
            },
          },
        },
        {
          if: {
            properties: {
              kind: {
                const: "invisible",
              },
            },
            required: [
              "kind",
            ],
          },
          then: {
            properties: {
              lineStyle: {
                type: "null",
              },
              spine: {
                type: "null",
              },
              fillMatrix: {
                type: "null",
              },
              colourId: {
                type: "null",
              },
            },
          },
        },
        {
          if: {
            properties: {
              kind: {
                const: "brush",
              },
            },
            required: [
              "kind",
            ],
          },
          then: {
            properties: {
              lineStyle: {
                type: "null",
              },
            },
          },
        },
      ],
    },
    else: {
      propertyNames: {
        enum: [
          "id",
          "type",
          "name",
        ],
      },
    },
  },
  StrokeKind: {
    oneOf: [
      {
        type: "string",
        const: "brush",
        description: "Filled outline.",
      },
      {
        type: "string",
        const: "pencil",
        description: "Centre line drawn with a line style.",
      },
      {
        type: "string",
        const: "invisible",
        description: "Never rendered; separates and closes zones.",
      },
    ],
    description: "Kind of a stroke (02 §3.2).",
  },
  Stub: {
    type: "object",
    description: "A stub standing for an object that is not expanded (10 §2.2). A stub has no `rev`.",
    properties: {
      id: {
        $ref: "#/$defs/Id",
        "x-since": "1.0",
      },
      colourId: {
        $ref: "#/$defs/Id",
        "x-since": "1.0",
      },
      type: {
        $ref: "#/$defs/ObjectType",
        "x-since": "1.0",
      },
      name: {
        type: "string",
        "x-since": "1.0",
      },
    },
    required: [
      "type",
    ],
    oneOf: [
      {
        required: [
          "id",
        ],
      },
      {
        required: [
          "colourId",
        ],
      },
    ],
    additionalProperties: false,
  },
  Swatch: {
    type: "object",
    additionalProperties: false,
    properties: {
      colourId: {
        $ref: "#/$defs/Id",
        description: "Immutable colour ID, unique across the project.",
        readOnly: true,
        "x-since": "1.0",
      },
      type: {
        type: "string",
        const: "swatch",
        readOnly: true,
        description: "Always `swatch`.",
        "x-since": "1.0",
      },
      rev: {
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
        description: "Revision of the last change.",
        readOnly: true,
        "x-since": "1.0",
      },
      name: {
        pattern: "^(?:[^\\u0000-\\u001F\\u007F\\s]|[^\\u0000-\\u001F\\u007F\\s][^\\u0000-\\u001F\\u007F]*[^\\u0000-\\u001F\\u007F\\s])$",
        description: "Label; names may repeat.",
        "x-command": "colour.swatch.set",
        minLength: 1,
        maxLength: 128,
        type: "string",
        "x-since": "1.0",
      },
      kind: {
        $ref: "#/$defs/SwatchKind",
        description: "Solid, gradient or texture; shared by all styles.",
        "x-command": "colour.swatch.set",
        "x-since": "1.0",
      },
      deformable: {
        type: "boolean",
        description: "Whether a gradient/texture fill follows bone deformation (03 §7.5).",
        "x-since": "1.0",
      },
      values: {
        type: "object",
        additionalProperties: false,
        patternProperties: {
          "^[A-Za-z0-9_-]{1,64}$": {
            $ref: "#/$defs/SwatchValue",
          },
        },
        description: "Exactly one value per style of the palette, keyed by style ID (keys in canonical order, ADR-0011 §3).",
        "x-command": "colour.swatch.set",
        "x-since": "1.0",
      },
      meta: {
        type: "object",
        additionalProperties: true,
        description: "Free-form metadata.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      ext: {
        $ref: "#/$defs/Ext",
        description: "Extension data.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      path: {
        type: "string",
        description: "Canonical path (10 §2.4).",
        "x-transient": true,
        readOnly: true,
        "x-since": "1.0",
      },
    },
    "x-since": "1.0",
    description: "A swatch (03 §1.4, §12.3). Its identifier field is `colourId` (10 §2.2); `id` is accepted as an alias on input.",
    "x-objectType": "swatch",
    required: [
      "colourId",
      "type",
    ],
    if: {
      required: [
        "rev",
      ],
    },
    then: {
      required: [
        "colourId",
        "type",
        "rev",
        "name",
        "kind",
        "deformable",
        "values",
      ],
    },
    else: {
      propertyNames: {
        enum: [
          "colourId",
          "type",
          "name",
        ],
      },
    },
  },
  SwatchKind: {
    oneOf: [
      {
        type: "string",
        const: "solid",
        description: "One RGBA colour per style.",
      },
      {
        type: "string",
        const: "linearGradient",
        description: "Linear gradient stops per style.",
      },
      {
        type: "string",
        const: "radialGradient",
        description: "Radial gradient stops per style.",
      },
      {
        type: "string",
        const: "texture",
        description: "Bitmap texture per style.",
      },
    ],
    description: "Kind of a swatch (03 §1.4).",
  },
  SwatchValue: {
    anyOf: [
      {
        $ref: "#/$defs/SolidValue",
        description: "A solid colour.",
      },
      {
        $ref: "#/$defs/GradientValue",
        description: "A gradient (linear or radial; the swatch `kind` says which).",
      },
      {
        $ref: "#/$defs/TextureValue",
        description: "A texture.",
      },
    ],
    description: "The value of a swatch in one style (03 §12.3): `{rgba}`, `{stops}` or `{image, tile}`.",
  },
  TemplateSummaryNode: {
    type: "object",
    additionalProperties: false,
    properties: {
      elementType: {
        type: "string",
        description: "Element type.",
        "x-since": "1.0",
      },
      name: {
        type: "string",
        description: "Element name.",
        "x-since": "1.0",
      },
      frameCount: {
        type: "integer",
        maximum: 2147483647,
        minimum: 0,
        description: "Frame count.",
        "x-since": "1.0",
      },
      children: {
        type: "array",
        items: {
          $ref: "#/$defs/TemplateSummaryNode",
        },
        description: "Children.",
        "x-since": "1.0",
      },
    },
    required: [
      "elementType",
      "name",
      "frameCount",
      "children",
    ],
    description: "A node of a template's element-tree summary (08 §6.3).",
  },
  TextRun: {
    type: "object",
    additionalProperties: false,
    properties: {
      start: {
        type: "integer",
        maximum: 2147483647,
        minimum: 0,
        description: "First code point.",
        "x-since": "1.0",
      },
      length: {
        type: "integer",
        maximum: 2147483647,
        minimum: 0,
        description: "Number of code points.",
        "x-since": "1.0",
      },
      font: {
        type: "string",
        description: "Font family name (kept even when the font is missing).",
        "x-since": "1.0",
      },
      size: {
        type: "number",
        "x-unit": "du",
        description: "Size in drawing units.",
        minimum: 0,
        "x-since": "1.0",
      },
      bold: {
        type: "boolean",
        description: "Bold.",
        "x-since": "1.0",
      },
      italic: {
        type: "boolean",
        description: "Italic.",
        "x-since": "1.0",
      },
      script: {
        $ref: "#/$defs/Script",
        description: "Script position.",
        "x-since": "1.0",
      },
      kerning: {
        type: "number",
        description: "Manual kerning.",
        "x-since": "1.0",
      },
      autoKerning: {
        type: "boolean",
        description: "Automatic kerning.",
        "x-since": "1.0",
      },
      colourId: {
        anyOf: [
          {
            $ref: "#/$defs/Id",
          },
          {
            type: "null",
          },
        ],
        description: "Fill colour.",
        "x-ref": "swatch",
        "x-since": "1.0",
      },
    },
    required: [
      "start",
      "length",
      "font",
      "size",
      "bold",
      "italic",
      "script",
      "kerning",
      "autoKerning",
      "colourId",
    ],
    description: "Character formatting of a run of a text node (10 §4.9).",
  },
  TextureValue: {
    type: "object",
    additionalProperties: false,
    properties: {
      image: {
        type: "string",
        description: "Package entry path of the image, e.g. `textures/fabric-day.png`.",
        minLength: 1,
        maxLength: 1024,
        "x-since": "1.0",
      },
      tile: {
        type: "boolean",
        description: "Repeat the image (true) or stretch it over the zone (false).",
        "x-since": "1.0",
      },
      width: {
        type: "integer",
        maximum: 2147483647,
        minimum: 1,
        description: "Informative pixel width.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      height: {
        type: "integer",
        maximum: 2147483647,
        minimum: 1,
        description: "Informative pixel height.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
    },
    required: [
      "image",
      "tile",
    ],
    description: "`{image, tile}`: a texture image entry (under `textures/`) and the tile flag; `width`/`height` informative.",
  },
  Timestamp: {
    type: "string",
    pattern: "^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}\\.[0-9]{3}Z$",
    description: "ISO-8601 UTC timestamp with milliseconds.",
  },
  Tip: {
    oneOf: [
      {
        type: "string",
        const: "round",
        description: "Round nib (default).",
      },
      {
        type: "string",
        const: "oval",
        description: "Oval nib.",
      },
      {
        type: "string",
        const: "square",
        description: "Square nib.",
      },
      {
        type: "string",
        const: "flat",
        description: "Flat nib.",
      },
    ],
    description: "Tip shape (02 §6.1).",
  },
  Transform: {
    type: "object",
    additionalProperties: false,
    properties: {
      offset: {
        $ref: "#/$defs/Point3",
        description: "Static position (fields). Camera default (0, 0, 16).",
        "x-unit": "field",
        "x-command": "anim.transform.set",
        "x-since": "1.0",
      },
      rotation: {
        type: "number",
        "x-command": "anim.transform.set",
        description: "Static rotation (degrees; camera: roll).",
        "x-unit": "degree",
        "x-since": "1.0",
      },
      skew: {
        type: "number",
        "x-command": "anim.transform.set",
        description: "Static skew, −89…89 degrees.",
        minimum: -89,
        maximum: 89,
        "x-unit": "degree",
        "x-since": "1.0",
      },
      scale: {
        $ref: "#/$defs/Scale2",
        description: "Static scale.",
        "x-command": "anim.transform.set",
        "x-since": "1.0",
      },
      keepProportions: {
        type: "boolean",
        description: "Keep the scale ratio when editing one value.",
        "x-since": "1.0",
      },
      rotationPivot: {
        "x-kindSpecific": true,
        description: "Elements: rotation pivot (fields).",
        "x-unit": "field",
        $ref: "#/$defs/Point2",
        "x-since": "1.0",
      },
      scalePivot: {
        "x-kindSpecific": true,
        description: "Elements: scale pivot (fields).",
        "x-unit": "field",
        $ref: "#/$defs/Point2",
        "x-since": "1.0",
      },
      rotateAboutPivot: {
        type: "boolean",
        description: "Elements: rotate about the rotation pivot.",
        "x-kindSpecific": true,
        "x-since": "1.0",
      },
      pivot: {
        "x-kindSpecific": true,
        description: "Pegs and skeletons: pivot (fields).",
        "x-unit": "field",
        $ref: "#/$defs/Point2",
        "x-since": "1.0",
      },
      applyPivotTo: {
        "x-kindSpecific": true,
        description: "Pegs and skeletons: what the pivot applies to.",
        $ref: "#/$defs/ApplyPivotTo",
        "x-since": "1.0",
      },
      useChildDrawingPivot: {
        type: "boolean",
        description: "Pegs: use the child drawing's pivot.",
        "x-kindSpecific": true,
        "x-since": "1.0",
      },
      lockDepth: {
        type: "boolean",
        description: "Constant Z for nodes with a motion path (05 §9.3).",
        "x-since": "1.0",
      },
      orientToPath: {
        type: "boolean",
        description: "Rotation follows the motion-path tangent (05 §8.3).",
        "x-since": "1.0",
      },
    },
    required: [
      "offset",
      "rotation",
      "skew",
      "scale",
      "keepProportions",
      "lockDepth",
      "orientToPath",
    ],
    description: "The static transform block (10 §4.6, 05 §1.5, §8). Pivot fields depend on the element type: drawing, annotation,\nimage and media elements have `rotationPivot`, `scalePivot`, `rotateAboutPivot`; pegs have `pivot`,\n`applyPivotTo`, `useChildDrawingPivot`; skeletons have `pivot`, `applyPivotTo`; cameras have none (ADR-0010).",
  },
  Transition: {
    type: "object",
    additionalProperties: false,
    properties: {
      id: {
        $ref: "#/$defs/Id",
        description: "Immutable ID.",
        readOnly: true,
        "x-since": "1.0",
      },
      type: {
        type: "string",
        const: "transition",
        readOnly: true,
        description: "Always `transition`.",
        "x-since": "1.0",
      },
      rev: {
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
        description: "Revision of the last change.",
        readOnly: true,
        "x-since": "1.0",
      },
      transitionType: {
        $ref: "#/$defs/TransitionType",
        description: "The transition type (08 `type`; SI-004).",
        "x-command": "storyline.transition.set",
        "x-since": "1.0",
      },
      durationLeft: {
        type: "integer",
        "x-command": "storyline.transition.set",
        minimum: 0,
        description: "Duration / left duration, 0–1000 frames (default 12).",
        maximum: 1000,
        "x-unit": "frame",
        "x-since": "1.0",
      },
      blankDuration: {
        type: "integer",
        "x-command": "storyline.transition.set",
        minimum: 0,
        description: "Blank duration (fade only), 0–1000 frames (default 0).",
        maximum: 1000,
        "x-unit": "frame",
        "x-since": "1.0",
      },
      durationRight: {
        type: "integer",
        "x-command": "storyline.transition.set",
        minimum: 0,
        description: "Right duration (fade only), 0–1000 frames (default 12).",
        maximum: 1000,
        "x-unit": "frame",
        "x-since": "1.0",
      },
      colour: {
        $ref: "#/$defs/Colour",
        description: "Fade colour (default opaque black).",
        "x-command": "storyline.transition.set",
        "x-since": "1.0",
      },
      angle: {
        type: "number",
        "x-command": "storyline.transition.set",
        description: "Angle for wipe, slide and clock wipe, 0–360 degrees (default 0).",
        minimum: 0,
        maximum: 360,
        "x-unit": "degree",
        "x-since": "1.0",
      },
      direction: {
        anyOf: [
          {
            $ref: "#/$defs/TransitionDirection",
          },
          {
            type: "null",
          },
        ],
        description: "Direction for slide and clock wipe; `null` until a type that uses it is chosen.",
        "x-command": "storyline.transition.set",
        "x-since": "1.0",
      },
      size: {
        type: "integer",
        "x-command": "storyline.transition.set",
        minimum: 5,
        description: "Checker size, 5–100 in steps of 5 (default 25).",
        maximum: 100,
        multipleOf: 5,
        "x-since": "1.0",
      },
      meta: {
        type: "object",
        additionalProperties: true,
        description: "Free-form metadata.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      ext: {
        $ref: "#/$defs/Ext",
        description: "Extension data.",
        "x-omitDefault": true,
        "x-since": "1.0",
      },
      path: {
        type: "string",
        description: "Canonical path (10 §2.4).",
        "x-transient": true,
        readOnly: true,
        "x-since": "1.0",
      },
    },
    "x-since": "1.0",
    description: "A transition at an edit point: `project.startTransition` or a scene's `outTransition` (10 §4.4). Fields not\nused by the current type are kept so that switching back restores them.",
    "x-objectType": "transition",
    required: [
      "id",
      "type",
    ],
    if: {
      required: [
        "rev",
      ],
    },
    then: {
      required: [
        "id",
        "type",
        "rev",
        "transitionType",
        "durationLeft",
        "blankDuration",
        "durationRight",
        "colour",
        "angle",
        "direction",
        "size",
      ],
    },
    else: {
      propertyNames: {
        enum: [
          "id",
          "type",
          "name",
        ],
      },
    },
  },
  TransitionDirection: {
    oneOf: [
      {
        type: "string",
        const: "in",
        description: "Slide in.",
      },
      {
        type: "string",
        const: "out",
        description: "Slide out.",
      },
      {
        type: "string",
        const: "clockwise",
        description: "Clock wipe clockwise.",
      },
      {
        type: "string",
        const: "counterclockwise",
        description: "Clock wipe counter-clockwise.",
      },
    ],
    description: "Transition direction (08 §13.1; type-dependent).",
  },
  TransitionType: {
    oneOf: [
      {
        type: "string",
        const: "none",
        description: "No transition (new edit points).",
      },
      {
        type: "string",
        const: "fade",
        description: "Fade through a colour.",
      },
      {
        type: "string",
        const: "crossDissolve",
        description: "Cross dissolve.",
      },
      {
        type: "string",
        const: "wipe",
        description: "Wipe.",
      },
      {
        type: "string",
        const: "slide",
        description: "Slide.",
      },
      {
        type: "string",
        const: "clockWipe",
        description: "Clock wipe.",
      },
      {
        type: "string",
        const: "checker",
        description: "Checkerboard.",
      },
    ],
    description: "Transition type (08 §13.1). Persisted as `transitionType` because `type` is the envelope field (SI-004).",
  },
  TreeNode: {
    type: "object",
    additionalProperties: false,
    properties: {
      id: {
        $ref: "#/$defs/Id",
        description: "The element.",
        "x-ref": "element",
        "x-since": "1.0",
      },
      children: {
        type: "array",
        items: {
          $ref: "#/$defs/TreeNode",
        },
        description: "Children, top to bottom as displayed in the Timeline.",
        "x-since": "1.0",
      },
    },
    required: [
      "id",
      "children",
    ],
    description: "A node of the scene tree: `{id, children}` (10 §4.3).",
  },
  VectorQuality: {
    oneOf: [
      {
        type: "string",
        const: "low",
        description: "Low.",
      },
      {
        type: "string",
        const: "medium",
        description: "Medium (default).",
      },
      {
        type: "string",
        const: "high",
        description: "High.",
      },
      {
        type: "string",
        const: "highest",
        description: "Highest.",
      },
    ],
    description: "Skeleton rendering quality.",
  },
  WidthMode: {
    oneOf: [
      {
        type: "string",
        const: "auto",
        description: "No wrapping.",
      },
      {
        type: "string",
        const: "fixed",
        description: "Word wrap at `width`.",
      },
    ],
    description: "Width mode of a text node.",
  },
  ZoneFill: {
    type: "object",
    additionalProperties: false,
    properties: {
      colourId: {
        anyOf: [
          {
            $ref: "#/$defs/Id",
          },
          {
            type: "null",
          },
        ],
        description: "Colour ID; `null` = unpainted.",
        "x-ref": "swatch",
        "x-since": "1.0",
      },
      matrix: {
        type: [
          "array",
          "null",
        ],
        items: {
          type: "number",
        },
        minItems: 6,
        maxItems: 6,
        description: "Gradient/texture placement; `null` for none.",
        "x-since": "1.0",
      },
    },
    description: "The fill of the zone on one side of a stroke (10 §4.9).",
    required: [
      "colourId",
      "matrix",
    ],
  },
};

/** Paragraph alignment. */
export type Alignment = 'left' | 'centre' | 'right' | 'justify';

/** Pivot application of pegs and skeletons (05 §2.3). */
export type ApplyPivotTo = 'rotation' | 'scaling' | 'both';

/** The project-wide registry entry of one media file (10 §4.16). */
export interface Asset {
  /** Immutable ID. */
  id: Id;
  /** Always `asset`. */
  type: 'asset';
  /** Revision of the last change. */
  rev: number | bigint;
  /** Informational name (file name by default). */
  name: string;
  /** Media kind. */
  kind: AssetKind;
  /** IANA media type, e.g. `audio/wav`. */
  mediaType: string;
  /** Embedded or linked. */
  storage: AssetStorage;
  /** Package entry path when embedded (`media/<assetId>.<ext>`); `null` when linked. */
  entry: string | null;
  /** The link when linked; `null` when embedded. */
  link: AssetLink | null;
  /** Size of the file bytes (64-bit). */
  size: number | bigint;
  /** Lower-case hex SHA-256 of the file bytes. */
  sha256: string;
  /** Import time. */
  importedAt: Timestamp;
  /** Where the file was imported from (informational). */
  sourcePath: string | null;
  /** Free-form metadata. */
  meta?: Record<string, unknown>;
  /** Extension data. */
  ext?: Ext;
  /** Canonical path (10 §2.4). */
  path?: string;
  info?: Record<string, unknown>;
  refs?: Id[];
  status?: 'ok' | 'missing' | 'changed' | 'unsupported';
}

/** A `Asset` reduced to its stub (10 §2.2). */
export interface AssetStub {
  id: Id;
  type: 'asset';
  name?: string;
}

/** Media kind of an asset. */
export type AssetKind = 'sound' | 'image' | 'movie' | 'vector';

/** The link of a `linked` asset (10 §4.16). Fields that do not apply to the link kind are `null`. */
export interface AssetLink {
  /** File or library item. */
  kind: LinkKind;
  /** Files: path relative to the folder containing the project. */
  path: string | null;
  /** Files: absolute path at link time. */
  absolutePath: string | null;
  /** Library items: library ID. */
  libraryId: Id | null;
  /** Library items: item ID. */
  itemId: Id | null;
  /** Library items: template version seen at link time (08 LIB-007). */
  versionId: string | null;
}

/** Where the bytes live (10 §6.7). */
export type AssetStorage = 'embedded' | 'linked';

/** A 2-D affine transform as axes and origin (06 §3.4 shadow transform). */
export interface AxesTransform {
  /** Image of the x axis. */
  xAxis: Point2;
  /** Image of the y axis. */
  yAxis: Point2;
  /** Origin (fields). */
  origin: Point2;
}

/** A binary payload stored as a package entry (10 §1.1). Never inlined in object JSON. */
export interface Blob {
  /** Package entry path. */
  entry: string;
  /** IANA media type. */
  mediaType: string;
  /** Size in bytes (64-bit: entries larger than 4 GiB are supported, 10 §5). */
  size: number | bigint;
  /** Lower-case hex SHA-256 of the bytes. */
  sha256: string;
}

/** Static parameters of a particle block (06 §3.8.3). Frames are absolute scene frames. */
export interface BlockStatic {
  /** Background drawn behind the particles. */
  background: ParticleBackground;
  /** First simulated frame (≤ firstFrame, ≥ firstFrame − 999; may be < 1 for pre-roll). */
  simulationStart: number;
  /** First displayed frame. */
  firstFrame: number;
  /** Frames the block lasts, ≥ 1. */
  duration: number;
  /** Particle cap (≤ 3000; 50 for fog and cloud). */
  maxParticles: number;
  /** Lifetime in frames: −1 (unlimited) or 0–512. */
  lifetimeFrames: number;
  /** Fade particles out. */
  fade: boolean;
  /** Region width 0–1999 particle units. */
  areaWidth: number;
  /** Region height 0–1999. */
  areaHeight: number;
  /** Region depth 0–1999. */
  areaDepth: number;
  /** Floor height 0–1999. */
  floorLevel: number;
}

/** Blur parameters (06 §3.5). */
export interface BlurParams {
  /** Uniform (true) or directional (false) blur (static). */
  radial: boolean;
  /** Radius 0–100 output pixels (animatable). */
  radius: number;
  /** Motion-blur direction 0–360 degrees (animatable). */
  angle: number;
}

/** Bone block (10 §4.6, 05 §13.1–§13.3). */
export interface BoneBlock {
  /** Rest pose. */
  rest: BoneRest;
  /** Zone of influence. */
  influence: BoneInfluence;
}

/** Zone of influence of a bone (05 §13.3). */
export interface BoneInfluence {
  /** Infinite or elliptical. */
  type: InfluenceType;
  /** Length radius (fields). */
  lengthRadius: number;
  /** Width radius (fields). */
  widthRadius: number;
  /** Explicit region polygon, or `null`. */
  region: Point2[] | null;
}

/** Rest pose of a bone (05 §13.1). */
export interface BoneRest {
  /** X (fields). */
  x: number;
  /** Y (fields). */
  y: number;
  /** Rotation (degrees). */
  rotation: number;
  /** Length (fields), > 0.01. */
  length: number;
}

/** Camera block (10 §4.6, 05 §12.3). */
export interface CameraBlock {
  /** Static or dynamic FOV. */
  fovMode: FovMode;
  /** FOV 1–179 degrees (default 41.1121). */
  fov: number;
}

/** An animation channel (10 §4.12). The `name` is the channel id (`rotation`, `scale.x`, …, or an effect parameter id) and equals its key in the element's `channels` map. A 1-D channel has `curveType` and `keys`; the motion-path channel `position` has `points` instead. */
export interface Channel {
  /** Immutable ID. */
  id: Id;
  /** Always `channel`. */
  type: 'channel';
  /** Revision of the last change. */
  rev: number | bigint;
  /** The channel id. */
  name: string;
  /** 1-D channels: curve type. */
  curveType?: CurveType;
  /** 1-D channels: keys sorted by frame, at least one. */
  keys?: (Key | KeyStub)[];
  /** Motion path: points in path order; first and last are keyframes. */
  points?: (MotionPoint | MotionPointStub)[];
  /** Free-form metadata. */
  meta?: Record<string, unknown>;
  /** Extension data. */
  ext?: Ext;
  /** Canonical path (10 §2.4). */
  path?: string;
}

/** A `Channel` reduced to its stub (10 §2.2). */
export interface ChannelStub {
  id: Id;
  type: 'channel';
  name?: string;
}

/** Clipping parameters (06 §3.3). */
export interface ClippingParams {
  /** Draw children outside the mask instead of inside. */
  inverted: boolean;
}

/** sRGB colour with straight alpha, #RRGGBBAA (written upper case; #RRGGBB accepted on input). */
export type Colour = string;

/** A recovery copy of one swatch used by a drawing (10 §4.11, 03 §11.2). */
export interface ColourRecoveryEntry {
  /** The colour ID. */
  colourId: Id;
  /** Owning palette ID. */
  paletteId: Id;
  /** Owning palette name. */
  paletteName: string;
  /** The palette's styles. */
  styles: RecoveryStyle[];
  /** The full swatch (all style values). */
  swatch: Swatch | SwatchStub;
}

/** Colour Transform parameters (06 §3.2). */
export interface ColourTransformParams {
  /** Red multiplier 0–1 (animatable). */
  multRed: number;
  /** Green multiplier 0–1 (animatable). */
  multGreen: number;
  /** Blue multiplier 0–1 (animatable). */
  multBlue: number;
  /** Alpha multiplier 0–1 (animatable). */
  multAlpha: number;
  /** Red offset −255…255 (animatable). */
  addRed: number;
  /** Green offset −255…255 (animatable). */
  addGreen: number;
  /** Blue offset −255…255 (animatable). */
  addBlue: number;
  /** Alpha offset −255…255 (animatable). */
  addAlpha: number;
  /** Composite the sub-tree first (static). */
  flatten: boolean;
}

/** Curve type of a 1-D channel (05 §10.3). */
export type CurveType = 'spline' | 'interpolation' | 'velocityBased';

/** A drawing (10 §4.8). Stored in its own package entry `banks/<bankId>/<drawingId>.json`. */
export interface Drawing {
  /** Immutable ID. */
  id: Id;
  /** Always `drawing`. */
  type: 'drawing';
  /** Revision of the last change (the drawing revision of 02 §3.12). */
  rev: number | bigint;
  /** Unique in the bank; default next unused integer. */
  name: string;
  /** Locked drawings reject content edits with `drawing.locked`. */
  locked: boolean;
  /** Drawing pivot (du). */
  pivot: Point2;
  /** Mouth shape for automatic lip-sync mapping. */
  mouthShape: MouthShape | null;
  /** Nodes, back to front. */
  nodes: (DrawingNode | DrawingNodeStub)[];
  /** One entry per colour ID the drawing uses (written on save). */
  colourRecovery: ColourRecoveryEntry[];
  /** Creation time. */
  createdAt: Timestamp;
  /** Free-form metadata. */
  meta?: Record<string, unknown>;
  /** Extension data. */
  ext?: Ext;
  /** Canonical path (10 §2.4). */
  path?: string;
  thumbnail?: Blob | null;
  usage?: ({
    elementId: Id;
    ranges: FrameRange[];
  })[];
  isEmpty?: boolean;
  zones?: (Record<string, unknown>)[];
}

/** A `Drawing` reduced to its stub (10 §2.2). */
export interface DrawingStub {
  id: Id;
  type: 'drawing';
  name?: string;
}

/** The drawings of one clone group (10 §4.8). Stored in `banks/<bankId>/bank.json` with drawing stubs. */
export interface DrawingBank {
  /** Immutable ID. */
  id: Id;
  /** Always `drawingBank`. */
  type: 'drawingBank';
  /** Revision of the last change. */
  rev: number | bigint;
  /** Drawings (unordered; written in natural name order). */
  drawings: (Drawing | DrawingStub)[];
  /** Free-form metadata. */
  meta?: Record<string, unknown>;
  /** Extension data. */
  ext?: Ext;
  /** Canonical path (10 §2.4). */
  path?: string;
  elementIds?: Id[];
}

/** A `DrawingBank` reduced to its stub (10 §2.2). */
export interface DrawingBankStub {
  id: Id;
  type: 'drawingBank';
}

/** A node of a drawing (10 §4.9): a layer (`hint`, `strokes`), a group (`children`), a text object (`transform`, `widthMode`, `width`, `text`, `runs`, `paragraphs`) or a bitmap (`image`, `width`, `height`, `transform`). Properties of other kinds are absent. */
export interface DrawingNode {
  /** ID, unique within the drawing (ID-03). */
  id: Id;
  /** Always `drawingNode`. */
  type: 'drawingNode';
  /** Revision of the last change. */
  rev: number | bigint;
  /** Optional name. */
  name?: string;
  /** Layer, group, text or bitmap; read-only. */
  kind: NodeKind;
  /** Layer: type hint. */
  hint?: LayerHint;
  /** Layer: strokes. */
  strokes?: (Stroke | StrokeStub)[];
  /** Group: child nodes, back to front. */
  children?: (DrawingNode | DrawingNodeStub)[];
  /** Text and bitmap: placement in drawing space. */
  transform?: number[];
  /** Text: auto or fixed width. */
  widthMode?: WidthMode;
  /** Text: wrap width in du (fixed mode); bitmap: pixel width 1–16384. */
  width?: number;
  /** Bitmap: pixel height 1–16384. */
  height?: number;
  /** Text: the characters (≤ 100 000). */
  text?: string;
  /** Text: character runs covering the text. */
  runs?: TextRun[];
  /** Text: paragraphs. */
  paragraphs?: Paragraph[];
  /** Bitmap: PNG pixels (8-bit RGBA, straight alpha). */
  image?: Blob;
  /** Free-form metadata. */
  meta?: Record<string, unknown>;
  /** Extension data. */
  ext?: Ext;
  /** Canonical path (10 §2.4). */
  path?: string;
}

/** A `DrawingNode` reduced to its stub (10 §2.2). */
export interface DrawingNodeStub {
  id: Id;
  type: 'drawingNode';
  name?: string;
}

/** Drop Shadow parameters (06 §3.4; all static). */
export interface DropShadowParams {
  /** How a parent Colour Transform applies. */
  parentColourTransformMode: ParentColourTransformMode;
  /** Shadow colour (alpha ignored). */
  colour: Colour;
  /** Opacity percent 0–100. */
  opacity: number;
  /** Draw only the shadow. */
  hideElements: boolean;
  /** Shadow transform in effect-local space. */
  transform: AxesTransform;
}

/** A dynamic block parameter: a constant `{value}` or `{keyframes}` (06 §8.4). */
export type DynamicParam = {
  /** The value. */
  value: number;
} | {
  /** Keyframes sorted by frame, at least one. */
  keyframes: FrameValue[];
};

/** Easing presets (05 §10.4). */
export type Easing = 'constant' | 'linear' | 'easeInOut' | 'fastInOut' | 'extremeEaseInOut' | 'extremeFastInOut' | 'slowAcceleration' | 'slowDeceleration';

/** An element of a scene (10 §4.5–§4.7). Stored in `scenes/<sceneId>/elements/<elementId>.json`. */
export interface Element {
  /** Immutable ID. */
  id: Id;
  /** Always `element`. */
  type: 'element';
  /** Revision of the last change. */
  rev: number | bigint;
  /** Unique in the scene. */
  name: string;
  /** Element type; read-only except through conversion commands. */
  elementType: ElementType;
  /** Non-cell elements: first frame of the bar (persisted). Cell-based elements: derived from the exposure (query results only). */
  start?: number;
  /** Non-cell elements: length of the bar (absent for cell-based elements: derived). */
  duration?: number;
  /** Loop count ≥ 1; 1 = not looped. */
  loops: number;
  /** Count or fill. */
  loopType: LoopType;
  /** Cycle or offset. */
  positionLoop: PositionLoop;
  /** Composite role. */
  role: Role;
  /** Deformable by a skeleton. */
  deformable: boolean;
  /** Shown in scene views, playback and export. */
  visibleInScene: boolean;
  /** Cell-based elements: shown in the Exposure Sheet and Drawing View. */
  visibleInXsheet?: boolean;
  /** Locked in scene views. */
  locked: boolean;
  /** Timeline display state of parents. */
  collapsed: boolean;
  /** Marker colour; `null` = category default. */
  markerColour: Colour | null;
  /** Background colour; `null` = category default. */
  backgroundColour: Colour | null;
  /** Element note (≤ 10 000 characters). */
  note?: string;
  /** Exposure Sheet column width, 20–1000 view pixels. */
  columnWidth: number;
  /** Cell-based: the drawing bank (shared by clones). */
  drawingBankId?: Id;
  /** Cell-based: canonical exposure runs from frame 1. */
  exposure?: ([Id | null, number])[];
  /** Cell-based: notes by frame. */
  cellNotes?: Record<string, string>;
  /** Image elements: import options (section 09; SI-008). */
  importOptions?: Record<string, unknown>;
  /** Sound: streamed playback. */
  streamed?: boolean;
  /** Sound: the Exposure Sheet shows mouths. */
  showMouthShapes?: boolean;
  /** Sound: placements sorted by `startFrame`, non-overlapping. */
  placements?: (SoundPlacement | SoundPlacementStub)[];
  /** Sound: one mapping per mapped drawing element. */
  lipSyncMappings?: LipSyncMapping[];
  /** Media: sequences, non-overlapping. */
  sequences?: (MediaSequence | MediaSequenceStub)[];
  /** Transformable elements: the static transform. */
  transform?: Transform;
  /** Pegs: export grouping. */
  exportGroup?: ExportGroup;
  /** Cameras: FOV settings. */
  camera?: CameraBlock;
  /** Skeletons: display and deformation settings. */
  skeleton?: SkeletonBlock;
  /** Bones: rest pose and influence. */
  bone?: BoneBlock;
  /** Effect parameter elements: the slot. */
  slot?: ParamSlot;
  /** Effects: static parameter values (exact shape per `elementType`). */
  params?: Record<string, unknown>;
  /** Particle effects: blocks, consecutive. */
  blocks?: (ParticleBlock | ParticleBlockStub)[];
  /** Animated channels by channel id. */
  channels?: Record<string, Channel | ChannelStub>;
  /** Creation time. */
  createdAt: Timestamp;
  /** Free-form metadata. */
  meta?: Record<string, unknown>;
  /** Extension data. */
  ext?: Ext;
  /** Canonical path (10 §2.4). */
  path?: string;
  parentId?: Id | null;
  childIds?: Id[];
  depth?: number;
  timelineIndex?: number;
  xsheetIndex?: number | null;
  cloneGroupId?: Id | null;
  cloneIds?: Id[];
  drawingCount?: number;
  hiddenByParent?: boolean;
  effectiveMarkerColour?: Colour;
}

/** A `Element` reduced to its stub (10 §2.2). */
export interface ElementStub {
  id: Id;
  type: 'element';
  name?: string;
}

/** Element types (10 §4.5). The 04 read model reports this value as `type`; the object model uses `elementType` (design decision D6). */
export type ElementType = 'drawing' | 'annotation' | 'image' | 'sound' | 'media' | 'peg' | 'camera' | 'skeleton' | 'bone' | 'effectParameter' | 'colourTransform' | 'clipping' | 'dropShadow' | 'blur' | 'shake' | 'particleNature' | 'particleImpact';

/** A gain point of a placement envelope (07 §9.5). */
export interface EnvelopePoint {
  /** Time within the source (seconds). */
  time: number;
  /** Left gain 0–1. */
  left: number;
  /** Right gain 0–1. */
  right: number;
}

/** Export-group block of pegs (05 §8.2, 09). */
export interface ExportGroup {
  /** Group the subtree on vector export. */
  groupOnVectorExport: boolean;
  /** Add a composite for the export group. */
  addComposite: boolean;
}

/** Extension data keyed by reverse-DNS namespace; unknown namespaces are preserved verbatim. */
export type Ext = Record<string, unknown>;

/** Camera FOV mode (05 §12.3). */
export type FovMode = 'static' | 'dynamic';

/** An inclusive frame range `{from, to}` (10 §1.1). */
export interface FrameRange {
  /** First frame (1-based). */
  from: number;
  /** Last frame (inclusive). */
  to: number;
}

/** A `{frame, value}` keyframe of a dynamic block parameter. */
export interface FrameValue {
  /** Absolute scene frame. */
  frame: number;
  /** Value. */
  value: number;
}

/** A gradient stop (03 §12.3). */
export interface GradientStop {
  /** Position along the gradient, 0–1. */
  position: number;
  /** Colour at the stop. */
  rgba: Colour;
}

/** `{stops}` with at least two stops sorted by position. */
export interface GradientValue {
  /** The stops. */
  stops: GradientStop[];
}

/** Handle mode. */
export type HandleMode = 'smooth' | 'broken';

/** A handle offset `{dt, dv}` (05 §10.3). */
export interface HandleOffset {
  /** Frame offset. */
  dt: number;
  /** Value offset. */
  dv: number;
}

/** Handles of a key. */
export interface Handles {
  /** Left handle. */
  left: HandleOffset;
  /** Right handle. */
  right: HandleOffset;
  /** Smooth or broken. */
  mode: HandleMode;
}

export type Id = string;

/** Influence type of a bone. */
export type InfluenceType = 'infinite' | 'elliptical';

/** Kind of a library item (10 §4.17). */
export type ItemKind = 'template' | 'image' | 'movie' | 'sound';

/** A key of a 1-D channel (10 §4.12). Addressed as `key:"<frame>"` or `key:@<id>`. */
export interface Key {
  /** ID, unique within the channel (ID-03). */
  id: Id;
  /** Always `key`. */
  type: 'key';
  /** Revision of the last change. */
  rev: number | bigint;
  /** Integer frame, unique in the channel. */
  frame: number;
  /** Value. */
  value: number;
  /** The segment starting here holds the value. */
  constantSegment: boolean;
  /** Spline handles. */
  handles: Handles;
  /** Easing preset of the segment starting here (interpolation curves). */
  easing: Easing | null;
  /** Free-form metadata. */
  meta?: Record<string, unknown>;
  /** Extension data. */
  ext?: Ext;
  /** Canonical path (10 §2.4). */
  path?: string;
}

/** A `Key` reduced to its stub (10 §2.2). */
export interface KeyStub {
  id: Id;
  type: 'key';
}

/** Type hint of a layer (02 §3.1). */
export type LayerHint = 'brush' | 'pencil';

/** A library (10 §4.17). In the document this is the project-local library (`kind: "local"`), stored under `library/library.json` in the package layout of 10 §6.11. */
export interface Library {
  /** Immutable, globally unique ID (08 LIB-002). */
  id: Id;
  /** Always `library`. */
  type: 'library';
  /** Revision of the last change. */
  rev: number | bigint;
  /** Library name. */
  name: string;
  /** Local, global, opened or starter. */
  kind: LibraryKind;
  /** Library format version (`MAJOR.MINOR`). */
  formatVersion: string;
  /** Top-level folders and items. */
  children: LibraryNode[];
  /** Free-form metadata. */
  meta?: Record<string, unknown>;
  /** Extension data. */
  ext?: Ext;
  /** Canonical path (10 §2.4). */
  path?: string;
  readOnly?: boolean;
  writeAccessHeldBy?: string | null;
  location?: string | null;
}

/** A `Library` reduced to its stub (10 §2.2). */
export interface LibraryStub {
  id: Id;
  type: 'library';
  name?: string;
}

/** A library folder (10 §4.17). */
export interface LibraryFolder {
  /** Immutable, globally unique ID (08 LIB-002). */
  id: Id;
  /** Always `libraryFolder`. */
  type: 'libraryFolder';
  /** Revision of the last change. */
  rev: number | bigint;
  /** Unique among siblings (case-insensitive). */
  name: string;
  /** Sub-folders and items. */
  children: LibraryNode[];
  /** Free-form metadata. */
  meta?: Record<string, unknown>;
  /** Extension data. */
  ext?: Ext;
  /** Canonical path (10 §2.4). */
  path?: string;
}

/** A `LibraryFolder` reduced to its stub (10 §2.2). */
export interface LibraryFolderStub {
  id: Id;
  type: 'libraryFolder';
  name?: string;
}

/** A library item (10 §4.17). Template metadata (08 §6.3) is present for templates only. */
export interface LibraryItem {
  /** Immutable, globally unique ID (08 LIB-002). */
  id: Id;
  /** Always `libraryItem`. */
  type: 'libraryItem';
  /** Revision of the last change. */
  rev: number | bigint;
  /** Display name, unique among siblings (case-insensitive). */
  name: string;
  /** Template, image, movie or sound. */
  itemKind: ItemKind;
  /** Stored file name relative to the library's `files/` folder. */
  file: string;
  /** Thumbnail, if any. */
  thumbnail: Blob | null;
  /** Templates: version ID, changed whenever the content changes (08 LIB-007). */
  versionId?: string;
  /** Templates: artist (08 LIB-010). */
  artist?: string;
  /** Templates: copyright. */
  copyright?: string;
  /** Templates: creation time. */
  createdAt?: Timestamp;
  /** Templates: modification time. */
  modifiedAt?: Timestamp;
  /** Templates: size in bytes. */
  sizeBytes?: number | bigint;
  /** Templates: template format version. */
  formatVersion?: string;
  /** Templates: source path or link (optional). */
  source?: string;
  /** Templates: element-tree summary. */
  summary?: TemplateSummaryNode[];
  /** Free-form metadata. */
  meta?: Record<string, unknown>;
  /** Extension data. */
  ext?: Ext;
  /** Canonical path (10 §2.4). */
  path?: string;
  broken?: boolean;
}

/** A `LibraryItem` reduced to its stub (10 §2.2). */
export interface LibraryItemStub {
  id: Id;
  type: 'libraryItem';
  name?: string;
}

/** Kind of a library (10 §4.17). */
export type LibraryKind = 'local' | 'global' | 'opened' | 'starter';

export type LibraryNode = LibraryFolder | LibraryFolderStub | LibraryItem | LibraryItemStub;

/** Line style of a pencil stroke. */
export interface LineStyle {
  /** Thickness in drawing units. */
  thickness: number;
  /** Line type. */
  lineType: LineType;
}

/** Line type of a pencil line style (02 §3.2; other types MAY be added). */
export type LineType = 'solid';

/** Kind of a link. */
export type LinkKind = 'file' | 'libraryItem';

/** A lip-sync mapping for one drawing element (10 §4.14). */
export interface LipSyncMapping {
  /** The mapped drawing element. */
  drawingElementId: Id;
  /** Shape → drawing. */
  mapping: MouthMapping;
  /** Re-apply automatically when the track changes. */
  autoReapply: boolean;
}

/** Loop type (05 §8.4). */
export type LoopType = 'count' | 'fill';

/** A run of frames of a media element showing one linked source (10 §4.15). */
export interface MediaSequence {
  /** Immutable ID. */
  id: Id;
  /** Always `mediaSequence`. */
  type: 'mediaSequence';
  /** Revision of the last change. */
  rev: number | bigint;
  /** A `linked` asset. */
  assetId: Id;
  /** First frame. */
  startFrame: number;
  /** Last frame. */
  stopFrame: number;
  /** Frame of the source shown at `startFrame`, ≥ 1. */
  linkOffset: number;
  /** Free-form metadata. */
  meta?: Record<string, unknown>;
  /** Extension data. */
  ext?: Ext;
  /** Canonical path (10 §2.4). */
  path?: string;
  sourceLength?: number;
  missing?: boolean;
}

/** A `MediaSequence` reduced to its stub (10 §2.2). */
export interface MediaSequenceStub {
  id: Id;
  type: 'mediaSequence';
}

/** A point of a motion path (10 §4.12): a keyframe (`frame` set) or a control point (`frame: null`). */
export interface MotionPoint {
  /** ID, unique within the channel (ID-03). */
  id: Id;
  /** Always `motionPoint`. */
  type: 'motionPoint';
  /** Revision of the last change. */
  rev: number | bigint;
  /** Keyframe frame (integer, unique); `null` = control point. */
  frame: number | null;
  /** Position in parent space (fields). */
  position: Point3;
  /** TCB tension, −1…1. */
  tension: number;
  /** TCB continuity, −1…1. */
  continuity: number;
  /** TCB bias, −1…1. */
  bias: number;
  /** The segment starting at this keyframe holds the position. */
  constantSegment: boolean;
  /** Constant Z for the segment starting here (05 §9.3). */
  lockDepth: boolean;
  /** Free-form metadata. */
  meta?: Record<string, unknown>;
  /** Extension data. */
  ext?: Ext;
  /** Canonical path (10 §2.4). */
  path?: string;
}

/** A `MotionPoint` reduced to its stub (10 §2.2). */
export interface MotionPointStub {
  id: Id;
  type: 'motionPoint';
}

/** Which drawing represents each mouth shape (07 §10.8). */
export interface MouthMapping {
  /** Drawing for A. */
  A: Id | null;
  /** Drawing for B. */
  B: Id | null;
  /** Drawing for C. */
  C: Id | null;
  /** Drawing for D. */
  D: Id | null;
  /** Drawing for E. */
  E: Id | null;
  /** Drawing for F. */
  F: Id | null;
  /** Drawing for G. */
  G: Id | null;
  /** Drawing for X. */
  X: Id | null;
}

/** Mouth shape letters (07 §10; 10 §4.8 `mouthShape`). */
export type MouthShape = 'A' | 'B' | 'C' | 'D' | 'E' | 'F' | 'G' | 'X';

/** A placement's lip-sync track (10 §4.14, 07 §10.3). */
export interface MouthTrack {
  /** One letter (`A`–`G`, `X`) per frame of the placement. */
  mouths: string;
  /** Placement-relative frame ranges set by hand or explicitly. */
  manual: FrameRange[];
  /** Optional display labels, keyed by placement-relative frame. */
  labels?: Record<string, string>;
  /** How the track was produced. */
  source: MouthTrackSource;
  /** Timing snapshot. */
  producedFor: ProducedFor;
  stale?: boolean;
}

/** How a mouth track was produced (10 §4.14). */
export type MouthTrackSource = 'detected' | 'transcript' | 'phonemes' | 'explicit';

/** Kind of a drawing node (02 §3.1). */
export type NodeKind = 'layer' | 'group' | 'text' | 'bitmap';

export type ObjectType = 'project' | 'palette' | 'paletteStyle' | 'swatch' | 'pen' | 'asset' | 'drawingBank' | 'drawing' | 'drawingNode' | 'stroke' | 'scene' | 'transition' | 'element' | 'channel' | 'key' | 'motionPoint' | 'particleBlock' | 'soundPlacement' | 'mediaSequence' | 'library' | 'libraryFolder' | 'libraryItem';

/** A palette (10 §4.13, 03 §1.3 and §12.3). */
export interface Palette {
  /** Immutable ID. */
  id: Id;
  /** Always `palette`. */
  type: 'palette';
  /** Revision of the last change. */
  rev: number | bigint;
  /** Unique in the project. */
  name: string;
  /** Styles in order; at least one. */
  styles: (PaletteStyle | PaletteStyleStub)[];
  /** Swatches in user order; may be empty. */
  swatches: (Swatch | SwatchStub)[];
  /** Internal palette never shown to the user (03 §1.2; SI-005). */
  hidden: boolean;
  /** Free-form metadata. */
  meta?: Record<string, unknown>;
  /** Extension data. */
  ext?: Ext;
  /** Canonical path (10 §2.4). */
  path?: string;
}

/** A `Palette` reduced to its stub (10 §2.2). */
export interface PaletteStub {
  id: Id;
  type: 'palette';
  name?: string;
}

/** A palette style: an alternative set of values for all swatches of its palette (03 §1.5). */
export interface PaletteStyle {
  /** Immutable ID. */
  id: Id;
  /** Always `paletteStyle`. */
  type: 'paletteStyle';
  /** Revision of the last change. */
  rev: number | bigint;
  /** Unique in the palette. */
  name: string;
  /** Free-form metadata. */
  meta?: Record<string, unknown>;
  /** Extension data. */
  ext?: Ext;
  /** Canonical path (10 §2.4). */
  path?: string;
}

/** A `PaletteStyle` reduced to its stub (10 §2.2). */
export interface PaletteStyleStub {
  id: Id;
  type: 'paletteStyle';
  name?: string;
}

/** Paragraph formatting of a text node (10 §4.9). */
export interface Paragraph {
  /** First code point. */
  start: number;
  /** Number of code points. */
  length: number;
  /** Alignment. */
  alignment: Alignment;
  /** Left margin (du). */
  marginLeft: number;
  /** Right margin (du). */
  marginRight: number;
  /** First-line indent (du). */
  indent: number;
  /** Line spacing factor. */
  lineSpacing: number;
}

/** Slot of an effect-parameter element (06 §2.3). */
export type ParamSlot = 'mask' | 'sprites' | 'bones' | 'skin';

/** How an inherited Colour Transform applies to a Drop Shadow (06 §3.4). */
export type ParentColourTransformMode = 'drawingsOnly' | 'drawingsAndShadow' | 'shadowOnly';

/** Background of a particle block (06 §3.8.3). */
export type ParticleBackground = 'transparent' | 'darkSkyGradient' | 'lightSkyGradient';

/** A simulation block of a particle effect (10 §4.6, 06 §3.8.2, §8.4). */
export interface ParticleBlock {
  /** Immutable ID. */
  id: Id;
  /** Always `particleBlock`. */
  type: 'particleBlock';
  /** Revision of the last change. */
  rev: number | bigint;
  /** Particle type (06 Block `type`; SI-004). */
  particleType: ParticleType;
  /** Sub-behaviour; `null` = type default. */
  variant: string | null;
  /** Last applied preset (informational). */
  presetId: string | null;
  /** Seed of all random choices, 0–2³²−1 (06 §3.8.3, §4.6: a full unsigned 32-bit range, stated explicitly). */
  seed: number;
  /** Built-in sprite set. */
  spriteSetId: string | null;
  /** Static parameters. */
  static: BlockStatic;
  /** Dynamic parameters by id (exactly the ids of 06 §3.8.4). */
  dynamic: Record<string, DynamicParam>;
  /** Free-form metadata. */
  meta?: Record<string, unknown>;
  /** Extension data. */
  ext?: Ext;
  /** Canonical path (10 §2.4). */
  path?: string;
  index?: number;
}

/** A `ParticleBlock` reduced to its stub (10 §2.2). */
export interface ParticleBlockStub {
  id: Id;
  type: 'particleBlock';
}

/** Particle effects have no effect-level parameters; their content is in `blocks` (06 §3.8). */
export type ParticleParams = Record<string, unknown>;

/** Particle types (06 §3.8.1). Persisted as `particleType` because `type` is the envelope field (SI-004). */
export type ParticleType = 'rain' | 'snow' | 'hail' | 'fog' | 'cloud' | 'eruption' | 'shatter' | 'magic' | 'explosion' | 'splash' | 'fire' | 'celestial';

/** A pen preset (10 §4.2). Unique name in the project. */
export interface Pen {
  /** Immutable ID. */
  id: Id;
  /** Always `pen`. */
  type: 'pen';
  /** Revision of the last change (10 REV-02). */
  rev: number | bigint;
  /** Unique in the project; default `Pen <n>`. */
  name: string;
  /** Brush or eraser pen (02 §6.1; SI-004). */
  kind: PenKind;
  /** Width at lowest pressure, 0–100 du; `minSize ≤ maxSize`. */
  minSize: number;
  /** Width at full pressure, 0–100 du. */
  maxSize: number;
  /** Curve-fitting tolerance and pressure smoothing, 0–10 (10 §4.2: int). */
  smoothness: number;
  /** Tip shape. */
  tip: Tip;
  /** Orientation of non-round tips, 0–360 degrees (02 §6.1; SI-004). */
  tipAngle: number;
  /** Colour bound to the pen; `null` = use the current colour. */
  colourId: Id | null;
  /** Free-form metadata. */
  meta?: Record<string, unknown>;
  /** Extension data. */
  ext?: Ext;
  /** Canonical path (10 §2.4). */
  path?: string;
}

/** A `Pen` reduced to its stub (10 §2.2). */
export interface PenStub {
  id: Id;
  type: 'pen';
  name?: string;
}

/** Tool family of a pen (02 §6.1 "Type"). Persisted as `kind` because `type` is the envelope field (SI-004). */
export type PenKind = 'brush' | 'eraser';

/** A 2-D point `{x, y}` (10 §1.1). */
export interface Point2 {
  /** X coordinate. */
  x: number;
  /** Y coordinate. */
  y: number;
}

/** A 3-D point `{x, y, z}` (10 §1.1). */
export interface Point3 {
  /** X coordinate. */
  x: number;
  /** Y coordinate. */
  y: number;
  /** Z coordinate. */
  z: number;
}

/** Position looping mode (05 §8.4). */
export type PositionLoop = 'cycle' | 'offset';

/** The timing a mouth track was produced for (10 §4.14). */
export interface ProducedFor {
  /** Asset. */
  assetId: Id;
  /** In-point (seconds). */
  inPoint: number;
  /** Out-point (seconds). */
  outPoint: number;
  /** Loop count. */
  loops: number;
  /** Stop frame. */
  stopFrame: number;
  /** Project frame rate. */
  frameRate: number;
}

/** The project (10 §4.1). Stored in `project.json` with stubs of palettes, drawing banks, scenes and the local library. */
export interface Project {
  /** Immutable ID. */
  id: Id;
  /** Always `project`. */
  type: 'project';
  /** Revision of the last change. */
  rev: number | bigint;
  /** Project name; default `Untitled`; also the default package file stem. */
  name: string;
  /** Creation time. */
  createdAt: Timestamp;
  /** Set by each save. */
  modifiedAt: Timestamp;
  /** Frames per second, 1–120; NTSC rates are exact as written. */
  frameRate: number;
  /** Camera (output) width, 16–8192 pixels. */
  cameraWidth: number;
  /** Camera (output) height, 16–8192 pixels. */
  cameraHeight: number;
  /** Whether template swatch values overwrite project values with the same colour IDs (03 §1.2; SI-005). */
  autoColourUpdate: boolean;
  /** Edit point before the first scene. */
  startTransition: Transition | TransitionStub;
  /** Palettes in display order; at least one. */
  palettes: (Palette | PaletteStub)[];
  /** Pens; at least one. */
  pens: (Pen | PenStub)[];
  /** Media assets. */
  assets: (Asset | AssetStub)[];
  /** Drawing banks (unordered; written in ID order). */
  drawingBanks: (DrawingBank | DrawingBankStub)[];
  /** Scenes in storyline order; at least one. */
  scenes: (Scene | SceneStub)[];
  /** The project-local library. */
  library: unknown;
  /** Free-form metadata. */
  meta?: Record<string, unknown>;
  /** Extension data. */
  ext?: Ext;
  /** Canonical path (10 §2.4). */
  path?: string;
  sceneStartFrames?: number[];
  globalLength?: number;
  thumbnail?: Blob | null;
}

/** A `Project` reduced to its stub (10 §2.2). */
export interface ProjectStub {
  id: Id;
  type: 'project';
  name?: string;
}

/** A style reference inside a colour-recovery entry. */
export interface RecoveryStyle {
  /** Style ID. */
  styleId: Id;
  /** Style name. */
  styleName: string;
}

/** Composite role (04 §6.1, 05 §8.1; SI-003: the superset of 04/05 and 10 §4.5). */
export type Role = 'normal' | 'foreground' | 'background' | 'envelope';

/** A 2-D scale `{x, y}`; `|s| ≥ 0.001`, negative values flip. */
export interface Scale2 {
  /** Horizontal factor. */
  x: number;
  /** Vertical factor. */
  y: number;
}

/** A scene (10 §4.3). Stored in `scenes/<sceneId>/scene.json` with element stubs. */
export interface Scene {
  /** Immutable ID. */
  id: Id;
  /** Always `scene`. */
  type: 'scene';
  /** Revision of the last change. */
  rev: number | bigint;
  /** Unique in the project; default `Scene-<n>`. */
  name: string;
  /** Hidden scenes are skipped by storyline playback and export. */
  visible: boolean;
  /** Background colour (default `#FFFFFFFF`). */
  backgroundColour: Colour;
  /** The current camera; must be a camera of this scene when it has one. */
  currentCameraId: Id | null;
  /** Active style per palette: exactly one entry per palette, in palette order. */
  paletteStyles: Record<string, Id>;
  /** Edit point after this scene. */
  outTransition: Transition | TransitionStub;
  /** Timeline order and hierarchy; every element exactly once. */
  tree: TreeNode[];
  /** Exposure Sheet order of the cell-based elements, back to front. */
  columnOrder: Id[];
  /** The elements (unordered; written in ID order). */
  elements: (Element | ElementStub)[];
  /** Free-form metadata. */
  meta?: Record<string, unknown>;
  /** Extension data. */
  ext?: Ext;
  /** Canonical path (10 §2.4). */
  path?: string;
  length?: number;
  paddedLength?: number;
}

/** A `Scene` reduced to its stub (10 §2.2). */
export interface SceneStub {
  id: Id;
  type: 'scene';
  name?: string;
}

/** Vertical script position of a text run. */
export type Script = 'normal' | 'subscript' | 'superscript';

/** Shake axes (06 §3.6). */
export type ShakeAxis = 'x' | 'y' | 'xy' | 'z' | 'xyz';

/** Shake parameters (06 §3.6). */
export interface ShakeParams {
  /** Displacement axes (static). */
  axis: ShakeAxis;
  /** Oscillation period 1–999 frames (static). */
  waveTime: number;
  /** Half-life 0.1–999 frames (static). */
  decayHalfLifeFrames: number;
  /** Peak displacement 0–100 fields (animatable). */
  amplitude: number;
}

/** Fills on the left and right side of a stroke, relative to point order. */
export interface Sides {
  /** Left side. */
  left: ZoneFill | null;
  /** Right side. */
  right: ZoneFill | null;
}

/** Skeleton block (05 §13.6). */
export interface SkeletonBlock {
  /** Bone colour; `null` = preference. */
  boneColour: Colour | null;
  /** Show the track. */
  showTrack: boolean;
  /** Show the root. */
  showRoot: boolean;
  /** Show bones. */
  showBones: boolean;
  /** Show influence. */
  showInfluence: boolean;
  /** Deformation quality. */
  vectorQuality: VectorQuality;
  /** Deformation on. */
  deformationEnabled: boolean;
}

/** `{rgba}`. */
export interface SolidValue {
  /** The colour. */
  rgba: Colour;
}

/** One use of a sound asset in a sound element (10 §4.14, 07 §1.2). */
export interface SoundPlacement {
  /** Immutable ID. */
  id: Id;
  /** Always `soundPlacement`. */
  type: 'soundPlacement';
  /** Revision of the last change. */
  rev: number | bigint;
  /** A `sound` asset, or a `movie` asset with audio. */
  assetId: Id;
  /** First frame. */
  startFrame: number;
  /** Last frame; `startFrame ≤ stopFrame ≤` natural end. */
  stopFrame: number;
  /** In-point in the source (seconds). */
  inPoint: number;
  /** Out-point in the source (seconds); `inPoint < outPoint`. */
  outPoint: number;
  /** Repetitions of the trimmed segment. */
  loops: number;
  /** Global gain −50…+50. */
  volume: number;
  /** Gain points, times ascending. */
  envelope: EnvelopePoint[];
  /** The placement's lip-sync track. */
  mouthTrack: MouthTrack | null;
  /** Free-form metadata. */
  meta?: Record<string, unknown>;
  /** Extension data. */
  ext?: Ext;
  /** Canonical path (10 §2.4). */
  path?: string;
  naturalEndFrame?: number;
}

/** A `SoundPlacement` reduced to its stub (10 §2.2). */
export interface SoundPlacementStub {
  id: Id;
  type: 'soundPlacement';
}

/** The spine of an unedited brush stroke (02 §3.2). */
export interface Spine {
  /** The fitted centre path. */
  points: number[][];
  /** Pressure map: `(t, p)` samples ordered by `t`. */
  pressure: number[][];
  /** Pen used. */
  pen: SpinePen;
}

/** The pen recorded on a brush spine (02 §3.2). */
export interface SpinePen {
  /** The pen used, if it still exists. */
  penId: Id | null;
  /** Tip shape. */
  tip: Tip;
  /** Minimum size (du). */
  minSize: number;
  /** Maximum size (du). */
  maxSize: number;
}

/** A stroke of a layer (10 §4.9, 02 §3.2). `lineStyle` is non-null for pencil strokes only; `spine` only for unedited brush strokes. */
export interface Stroke {
  /** ID, unique within the drawing (ID-03). */
  id: Id;
  /** Always `stroke`. */
  type: 'stroke';
  /** Revision of the last change. */
  rev: number | bigint;
  /** Brush, pencil or invisible. */
  kind: StrokeKind;
  /** Whether the path is closed (brush outlines are). */
  closed: boolean;
  /** Anchors with handle offsets; at least 2 (1 for a closed dab). */
  points: number[][];
  /** Brush: fill colour; pencil: line colour; `null` for invisible strokes. */
  colourId: Id | null;
  /** Placement of a gradient/texture colour space for brush strokes. */
  fillMatrix: number[] | null;
  /** Pencil strokes only. */
  lineStyle: LineStyle | null;
  /** Brush strokes while unedited. */
  spine: Spine | null;
  /** Zone fills on each side. */
  sides: Sides;
  /** Free-form metadata. */
  meta?: Record<string, unknown>;
  /** Extension data. */
  ext?: Ext;
  /** Canonical path (10 §2.4). */
  path?: string;
}

/** A `Stroke` reduced to its stub (10 §2.2). */
export interface StrokeStub {
  id: Id;
  type: 'stroke';
}

/** Kind of a stroke (02 §3.2). */
export type StrokeKind = 'brush' | 'pencil' | 'invisible';

/** A stub standing for an object that is not expanded (10 §2.2). A stub has no `rev`. */
export interface Stub {
  id?: Id;
  colourId?: Id;
  type: ObjectType;
  name?: string;
}

/** A swatch (03 §1.4, §12.3). Its identifier field is `colourId` (10 §2.2); `id` is accepted as an alias on input. */
export interface Swatch {
  /** Immutable colour ID, unique across the project. */
  colourId: Id;
  /** Always `swatch`. */
  type: 'swatch';
  /** Revision of the last change. */
  rev: number | bigint;
  /** Label; names may repeat. */
  name: string;
  /** Solid, gradient or texture; shared by all styles. */
  kind: SwatchKind;
  /** Whether a gradient/texture fill follows bone deformation (03 §7.5). */
  deformable: boolean;
  /** Exactly one value per style of the palette, keyed by style ID (keys in canonical order, ADR-0011 §3). */
  values: Record<string, SwatchValue>;
  /** Free-form metadata. */
  meta?: Record<string, unknown>;
  /** Extension data. */
  ext?: Ext;
  /** Canonical path (10 §2.4). */
  path?: string;
}

/** A `Swatch` reduced to its stub (10 §2.2). */
export interface SwatchStub {
  colourId: Id;
  type: 'swatch';
  name?: string;
}

/** Kind of a swatch (03 §1.4). */
export type SwatchKind = 'solid' | 'linearGradient' | 'radialGradient' | 'texture';

/** The value of a swatch in one style (03 §12.3): `{rgba}`, `{stops}` or `{image, tile}`. */
export type SwatchValue = SolidValue | GradientValue | TextureValue;

/** A node of a template's element-tree summary (08 §6.3). */
export interface TemplateSummaryNode {
  /** Element type. */
  elementType: string;
  /** Element name. */
  name: string;
  /** Frame count. */
  frameCount: number;
  /** Children. */
  children: TemplateSummaryNode[];
}

/** Character formatting of a run of a text node (10 §4.9). */
export interface TextRun {
  /** First code point. */
  start: number;
  /** Number of code points. */
  length: number;
  /** Font family name (kept even when the font is missing). */
  font: string;
  /** Size in drawing units. */
  size: number;
  /** Bold. */
  bold: boolean;
  /** Italic. */
  italic: boolean;
  /** Script position. */
  script: Script;
  /** Manual kerning. */
  kerning: number;
  /** Automatic kerning. */
  autoKerning: boolean;
  /** Fill colour. */
  colourId: Id | null;
}

/** `{image, tile}`: a texture image entry (under `textures/`) and the tile flag; `width`/`height` informative. */
export interface TextureValue {
  /** Package entry path of the image, e.g. `textures/fabric-day.png`. */
  image: string;
  /** Repeat the image (true) or stretch it over the zone (false). */
  tile: boolean;
  /** Informative pixel width. */
  width?: number;
  /** Informative pixel height. */
  height?: number;
}

/** ISO-8601 UTC timestamp with milliseconds. */
export type Timestamp = string;

/** Tip shape (02 §6.1). */
export type Tip = 'round' | 'oval' | 'square' | 'flat';

/** The static transform block (10 §4.6, 05 §1.5, §8). Pivot fields depend on the element type: drawing, annotation, image and media elements have `rotationPivot`, `scalePivot`, `rotateAboutPivot`; pegs have `pivot`, `applyPivotTo`, `useChildDrawingPivot`; skeletons have `pivot`, `applyPivotTo`; cameras have none (ADR-0010). */
export interface Transform {
  /** Static position (fields). Camera default (0, 0, 16). */
  offset: Point3;
  /** Static rotation (degrees; camera: roll). */
  rotation: number;
  /** Static skew, −89…89 degrees. */
  skew: number;
  /** Static scale. */
  scale: Scale2;
  /** Keep the scale ratio when editing one value. */
  keepProportions: boolean;
  /** Elements: rotation pivot (fields). */
  rotationPivot?: Point2;
  /** Elements: scale pivot (fields). */
  scalePivot?: Point2;
  /** Elements: rotate about the rotation pivot. */
  rotateAboutPivot?: boolean;
  /** Pegs and skeletons: pivot (fields). */
  pivot?: Point2;
  /** Pegs and skeletons: what the pivot applies to. */
  applyPivotTo?: ApplyPivotTo;
  /** Pegs: use the child drawing's pivot. */
  useChildDrawingPivot?: boolean;
  /** Constant Z for nodes with a motion path (05 §9.3). */
  lockDepth: boolean;
  /** Rotation follows the motion-path tangent (05 §8.3). */
  orientToPath: boolean;
}

/** A transition at an edit point: `project.startTransition` or a scene's `outTransition` (10 §4.4). Fields not used by the current type are kept so that switching back restores them. */
export interface Transition {
  /** Immutable ID. */
  id: Id;
  /** Always `transition`. */
  type: 'transition';
  /** Revision of the last change. */
  rev: number | bigint;
  /** The transition type (08 `type`; SI-004). */
  transitionType: TransitionType;
  /** Duration / left duration, 0–1000 frames (default 12). */
  durationLeft: number;
  /** Blank duration (fade only), 0–1000 frames (default 0). */
  blankDuration: number;
  /** Right duration (fade only), 0–1000 frames (default 12). */
  durationRight: number;
  /** Fade colour (default opaque black). */
  colour: Colour;
  /** Angle for wipe, slide and clock wipe, 0–360 degrees (default 0). */
  angle: number;
  /** Direction for slide and clock wipe; `null` until a type that uses it is chosen. */
  direction: TransitionDirection | null;
  /** Checker size, 5–100 in steps of 5 (default 25). */
  size: number;
  /** Free-form metadata. */
  meta?: Record<string, unknown>;
  /** Extension data. */
  ext?: Ext;
  /** Canonical path (10 §2.4). */
  path?: string;
}

/** A `Transition` reduced to its stub (10 §2.2). */
export interface TransitionStub {
  id: Id;
  type: 'transition';
}

/** Transition direction (08 §13.1; type-dependent). */
export type TransitionDirection = 'in' | 'out' | 'clockwise' | 'counterclockwise';

/** Transition type (08 §13.1). Persisted as `transitionType` because `type` is the envelope field (SI-004). */
export type TransitionType = 'none' | 'fade' | 'crossDissolve' | 'wipe' | 'slide' | 'clockWipe' | 'checker';

/** A node of the scene tree: `{id, children}` (10 §4.3). */
export interface TreeNode {
  /** The element. */
  id: Id;
  /** Children, top to bottom as displayed in the Timeline. */
  children: TreeNode[];
}

/** Skeleton rendering quality. */
export type VectorQuality = 'low' | 'medium' | 'high' | 'highest';

/** Width mode of a text node. */
export type WidthMode = 'auto' | 'fixed';

/** The fill of the zone on one side of a stroke (10 §4.9). */
export interface ZoneFill {
  /** Colour ID; `null` = unpainted. */
  colourId: Id | null;
  /** Gradient/texture placement; `null` for none. */
  matrix: number[] | null;
}
