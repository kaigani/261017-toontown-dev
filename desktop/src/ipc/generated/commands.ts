// GENERATED FILE: do not edit.
// Source: the command declarations (crates/executor/src/commands, crates/engine/src/commands.rs).
// Regenerate: TT_UPDATE_GOLDEN=1 cargo test -p tt-engine --test frontend_schemas
import type { Schema } from '../schema';

export const COMMANDS_REGISTRY_VERSION = "0.1.0";

export const COMMAND_INDEX = {
  "agent.activity.export": {
    kind: "action",
    params: "agent.activity.export.params",
    result: "agent.activity.export.result",
    examples: [
      {
        path: "/tmp/activity.json",
        format: "batch",
      },
    ],
    milestone: "M1 C1.4",
  },
  "agent.activity.list": {
    kind: "query",
    params: "agent.activity.list.params",
    result: "agent.activity.list.result",
    examples: [
      {
        limit: 20,
      },
    ],
    milestone: "M1 C1.4",
  },
  "agent.activity.persist.set": {
    kind: "toggle",
    params: "agent.activity.persist.set.params",
    result: "agent.activity.persist.set.result",
    examples: [
      {
        enabled: true,
      },
    ],
    milestone: "M1 C1.4",
  },
  "agent.confirmation.requested": {
    kind: "event",
    params: "agent.confirmation.requested.params",
    result: "agent.confirmation.requested.result",
    examples: [
      {},
    ],
    milestone: "M1 C1.4",
    event: true,
  },
  "agent.confirmation.respond": {
    kind: "ui",
    params: "agent.confirmation.respond.params",
    result: "agent.confirmation.respond.result",
    examples: [
      {
        confirmationId: "cf_1",
        accept: true,
      },
    ],
    milestone: "M1 C1.4",
  },
  "agent.policy.defaults.set": {
    kind: "ui",
    params: "agent.policy.defaults.set.params",
    result: "agent.policy.defaults.set.result",
    examples: [
      {
        policy: "assist",
        maxJobs: 2,
      },
    ],
    milestone: "M1 C1.4",
  },
  "agent.server.set": {
    kind: "ui",
    params: "agent.server.set.params",
    result: "agent.server.set.result",
    examples: [
      {
        enabled: true,
      },
    ],
    milestone: "M1 C1.4",
  },
  "agent.server.status": {
    kind: "query",
    params: "agent.server.status.params",
    result: "agent.server.status.result",
    examples: [
      {},
    ],
    milestone: "M1 C1.4",
  },
  "agent.session.connected": {
    kind: "event",
    params: "agent.session.connected.params",
    result: "agent.session.connected.result",
    examples: [
      {},
    ],
    milestone: "M1 C1.4",
    event: true,
  },
  "agent.session.disconnect": {
    kind: "ui",
    params: "agent.session.disconnect.params",
    result: "agent.session.disconnect.result",
    examples: [
      {
        sessionId: "a1",
      },
    ],
    milestone: "M1 C1.4",
  },
  "agent.session.disconnected": {
    kind: "event",
    params: "agent.session.disconnected.params",
    result: "agent.session.disconnected.result",
    examples: [
      {},
    ],
    milestone: "M1 C1.4",
    event: true,
  },
  "agent.session.grant": {
    kind: "ui",
    params: "agent.session.grant.params",
    result: "agent.session.grant.result",
    examples: [
      {
        sessionId: "a1",
        policy: "assist",
        scopes: [
          "read",
          "edit",
        ],
      },
    ],
    milestone: "M1 C1.4",
  },
  "agent.session.pause": {
    kind: "ui",
    params: "agent.session.pause.params",
    result: "agent.session.pause.result",
    examples: [
      {},
    ],
    milestone: "M1 C1.4",
  },
  "agent.session.resume": {
    kind: "ui",
    params: "agent.session.resume.params",
    result: "agent.session.resume.result",
    examples: [
      {},
    ],
    milestone: "M1 C1.4",
  },
  "agent.token.regenerate": {
    kind: "ui",
    params: "agent.token.regenerate.params",
    result: "agent.token.regenerate.result",
    examples: [
      {},
    ],
    milestone: "M1 C1.4",
  },
  "agent.typedTools.set": {
    kind: "ui",
    params: "agent.typedTools.set.params",
    result: "agent.typedTools.set.result",
    examples: [
      {
        enabled: true,
        namespaces: [
          "doc",
          "edit",
        ],
      },
    ],
    milestone: "M1 C1.4",
  },
  "app.context.set": {
    kind: "ui",
    params: "app.context.set.params",
    result: "app.context.set.result",
    examples: [
      {
        sceneId: "scene:#1",
        frame: 12,
      },
      {
        elementId: "scene:#1/element:#1",
      },
    ],
  },
  "app.currentElement.move": {
    kind: "ui",
    params: "app.currentElement.move.params",
    result: "app.currentElement.move.result",
    examples: [
      {
        direction: "next",
        order: "timeline",
      },
    ],
  },
  "app.currentElement.set": {
    kind: "ui",
    params: "app.currentElement.set.params",
    result: "app.currentElement.set.result",
    examples: [
      {
        elementId: "scene:#1/element:#1",
      },
    ],
  },
  "app.frame.move": {
    kind: "ui",
    params: "app.frame.move.params",
    result: "app.frame.move.result",
    examples: [
      {
        direction: "next",
      },
      {
        direction: "previous",
        unit: "drawing",
      },
    ],
  },
  "app.frame.set": {
    kind: "ui",
    params: "app.frame.set.params",
    result: "app.frame.set.result",
    examples: [
      {
        frame: 1,
      },
    ],
  },
  "app.info.get": {
    kind: "query",
    params: "app.info.get.params",
    result: "app.info.get.result",
    examples: [
      {},
    ],
    milestone: "M1 C1.4",
  },
  "app.mode.set": {
    kind: "ui",
    params: "app.mode.set.params",
    result: "app.mode.set.result",
    examples: [
      {
        mode: "drawing",
      },
    ],
  },
  "app.quit": {
    kind: "action",
    params: "app.quit.params",
    result: "app.quit.result",
    examples: [
      {
        onUnsaved: "discard",
      },
    ],
  },
  "app.state.changed": {
    kind: "event",
    params: "app.state.changed.params",
    result: "app.state.changed.result",
    examples: [
      {},
    ],
    event: true,
  },
  "app.state.get": {
    kind: "query",
    params: "app.state.get.params",
    result: "app.state.get.result",
    examples: [
      {},
    ],
  },
  "checkpoint.create": {
    kind: "ui",
    params: "checkpoint.create.params",
    result: "checkpoint.create.result",
    examples: [
      {
        name: "experiment-1",
      },
    ],
  },
  "checkpoint.delete": {
    kind: "ui",
    params: "checkpoint.delete.params",
    result: "checkpoint.delete.result",
    examples: [
      {
        name: "before-walk",
      },
    ],
  },
  "checkpoint.list": {
    kind: "query",
    params: "checkpoint.list.params",
    result: "checkpoint.list.result",
    examples: [
      {},
    ],
  },
  "checkpoint.restore": {
    kind: "action",
    params: "checkpoint.restore.params",
    result: "checkpoint.restore.result",
    examples: [
      {
        name: "before-walk",
      },
    ],
  },
  "doc.asset.embed": {
    kind: "action",
    params: "doc.asset.embed.params",
    result: "doc.asset.embed.result",
    examples: [
      {
        all: true,
      },
    ],
    milestone: "M1 C1.3",
  },
  "doc.asset.export": {
    kind: "query",
    params: "doc.asset.export.params",
    result: "doc.asset.export.result",
    examples: [
      {
        assetId: "as_1",
        path: "/tmp/sound.wav",
      },
    ],
    milestone: "M1 C1.3",
  },
  "doc.asset.list": {
    kind: "query",
    params: "doc.asset.list.params",
    result: "doc.asset.list.result",
    examples: [
      {
        status: "missing",
      },
    ],
    milestone: "M1 C1.3",
  },
  "doc.asset.relink": {
    kind: "action",
    params: "doc.asset.relink.params",
    result: "doc.asset.relink.result",
    examples: [
      {
        searchFolder: "/tmp/media",
      },
    ],
    milestone: "M1 C1.3",
  },
  "doc.asset.remove": {
    kind: "action",
    params: "doc.asset.remove.params",
    result: "doc.asset.remove.result",
    examples: [
      {
        unused: true,
      },
    ],
    milestone: "M1 C1.3",
  },
  "doc.asset.status.changed": {
    kind: "event",
    params: "doc.asset.status.changed.params",
    result: "doc.asset.status.changed.result",
    examples: [
      {},
    ],
    milestone: "M1 C1.3",
    event: true,
  },
  "doc.blob.get": {
    kind: "query",
    params: "doc.blob.get.params",
    result: "doc.blob.get.result",
    examples: [
      {
        entry: "thumbnail.png",
      },
    ],
    milestone: "M1 C1.3",
  },
  "doc.changed": {
    kind: "event",
    params: "doc.changed.params",
    result: "doc.changed.result",
    examples: [
      {},
    ],
    event: true,
  },
  "doc.diff": {
    kind: "query",
    params: "doc.diff.params",
    result: "doc.diff.result",
    examples: [
      {
        sinceRev: 1,
        detail: "summary",
      },
    ],
  },
  "doc.export.json": {
    kind: "query",
    params: "doc.export.json.params",
    result: "doc.export.json.result",
    examples: [
      {
        scope: "scenes",
        refs: [
          "scene:#1",
        ],
        binaries: "omit",
      },
    ],
    milestone: "M1 C1.3",
  },
  "doc.import.json": {
    kind: "action",
    params: "doc.import.json.params",
    result: "doc.import.json.result",
    examples: [
      {
        source: "/tmp/bundle.json",
        dryRun: true,
      },
    ],
    milestone: "M1 C1.3",
  },
  "doc.loadReport.get": {
    kind: "query",
    params: "doc.loadReport.get.params",
    result: "doc.loadReport.get.result",
    examples: [
      {},
    ],
    milestone: "M1 C1.3",
  },
  "doc.object.find": {
    kind: "query",
    params: "doc.object.find.params",
    result: "doc.object.find.result",
    examples: [
      {
        type: "element",
        where: {
          elementType: {
            op: "in",
            value: [
              "drawing",
              "image",
            ],
          },
        },
      },
      {
        type: "stroke",
        within: "drawingBank:#1/drawing:#1",
        limit: 10,
      },
      {
        name: "B*",
      },
    ],
  },
  "doc.object.get": {
    kind: "query",
    params: "doc.object.get.params",
    result: "doc.object.get.result",
    examples: [
      {
        ref: "scene:#1",
        depth: 1,
      },
      {
        ref: "@current",
        include: [
          "derived",
        ],
      },
      {
        refs: [
          "scene:#1",
          "palette:#1",
        ],
        fields: [
          "name",
        ],
      },
    ],
  },
  "doc.object.patch": {
    kind: "action",
    params: "doc.object.patch.params",
    result: "doc.object.patch.result",
    examples: [
      {
        targets: [
          "scene:#1",
        ],
        props: {
          meta: {
            status: "layout",
          },
        },
      },
      {
        targets: [
          "drawingBank:#1/drawing:#1",
        ],
        ops: [
          {
            op: "replace",
            path: "/mouthShape",
            value: "A",
          },
        ],
        label: "Set mouth shape",
      },
      {
        targets: [
          "pen:#1",
        ],
        props: {
          tip: "square",
          tipAngle: 30,
        },
        dryRun: true,
      },
      {
        targets: [
          "scene:#1/element:#1",
        ],
        props: {
          role: "foreground",
        },
        dryRun: true,
      },
    ],
  },
  "doc.object.refs": {
    kind: "query",
    params: "doc.object.refs.params",
    result: "doc.object.refs.result",
    examples: [
      {
        ref: "palette:#1",
      },
    ],
    milestone: "M1 C1.3",
  },
  "doc.object.resolve": {
    kind: "query",
    params: "doc.object.resolve.params",
    result: "doc.object.resolve.result",
    examples: [
      {
        refs: [
          "scene:#1",
          "@current",
        ],
      },
    ],
  },
  "doc.package.changedOnDisk": {
    kind: "event",
    params: "doc.package.changedOnDisk.params",
    result: "doc.package.changedOnDisk.result",
    examples: [
      {},
    ],
    milestone: "M1 C1.3",
    event: true,
  },
  "doc.package.export": {
    kind: "job",
    params: "doc.package.export.params",
    result: "doc.package.export.result",
    examples: [
      {
        path: "/tmp/Short",
        form: "folder",
      },
    ],
    milestone: "M1 C1.3",
  },
  "doc.package.get": {
    kind: "query",
    params: "doc.package.get.params",
    result: "doc.package.get.result",
    examples: [
      {
        path: "/tmp/Short.animpkg",
      },
    ],
    milestone: "M1 C1.3",
  },
  "doc.repair": {
    kind: "action",
    params: "doc.repair.params",
    result: "doc.repair.result",
    examples: [
      {
        codes: [
          "ref.dangling",
        ],
        dryRun: true,
      },
    ],
    milestone: "M1 C1.3",
  },
  "doc.schema.get": {
    kind: "query",
    params: "doc.schema.get.params",
    result: "doc.schema.get.result",
    examples: [
      {
        type: "element",
      },
      {
        type: "pen",
        property: "/tip",
      },
    ],
  },
  "doc.schema.list": {
    kind: "query",
    params: "doc.schema.list.params",
    result: "doc.schema.list.result",
    examples: [
      {},
    ],
  },
  "doc.schema.validate": {
    kind: "query",
    params: "doc.schema.validate.params",
    result: "doc.schema.validate.result",
    examples: [
      {
        type: "pen",
        object: {
          name: "Ink",
        },
        mode: "create",
      },
    ],
    milestone: "M1 C1.3",
  },
  "doc.state.get": {
    kind: "query",
    params: "doc.state.get.params",
    result: "doc.state.get.result",
    examples: [
      {},
    ],
  },
  "doc.validate": {
    kind: "query",
    params: "doc.validate.params",
    result: "doc.validate.result",
    examples: [
      {
        minSeverity: "error",
      },
    ],
    milestone: "M1 C1.3",
  },
  "edit.clipboard.changed": {
    kind: "event",
    params: "edit.clipboard.changed.params",
    result: "edit.clipboard.changed.result",
    examples: [
      {},
    ],
    event: true,
  },
  "edit.clipboard.get": {
    kind: "query",
    params: "edit.clipboard.get.params",
    result: "edit.clipboard.get.result",
    examples: [
      {
        includeData: false,
      },
    ],
  },
  "edit.clipboard.set": {
    kind: "ui",
    params: "edit.clipboard.set.params",
    result: "edit.clipboard.set.result",
    examples: [
      {
        content: {
          kind: "elements",
          sourceContext: "staging",
          itemCount: 2,
          items: [
            {
              elementId: "e1",
            },
            {
              elementId: "e2",
            },
          ],
          options: null,
        },
      },
    ],
  },
  "edit.copy": {
    kind: "ui",
    params: "edit.copy.params",
    result: "edit.copy.result",
    examples: [
      {},
    ],
    milestone: "M4 C4.1",
  },
  "edit.cut": {
    kind: "action",
    params: "edit.cut.params",
    result: "edit.cut.result",
    examples: [
      {},
    ],
    milestone: "M4 C4.1",
  },
  "edit.cutSpecial": {
    kind: "action",
    params: "edit.cutSpecial.params",
    result: "edit.cutSpecial.result",
    examples: [
      {
        options: {
          createDeleteExposure: false,
        },
      },
    ],
    milestone: "M4 C4.1",
  },
  "edit.delete": {
    kind: "action",
    params: "edit.delete.params",
    result: "edit.delete.result",
    examples: [
      {},
    ],
    milestone: "M4 C4.1",
  },
  "edit.deselectAll": {
    kind: "ui",
    params: "edit.deselectAll.params",
    result: "edit.deselectAll.result",
    examples: [
      {},
    ],
  },
  "edit.history.changed": {
    kind: "event",
    params: "edit.history.changed.params",
    result: "edit.history.changed.result",
    examples: [
      {},
    ],
    event: true,
  },
  "edit.history.get": {
    kind: "query",
    params: "edit.history.get.params",
    result: "edit.history.get.result",
    examples: [
      {
        limit: 20,
      },
    ],
  },
  "edit.history.move": {
    kind: "action",
    params: "edit.history.move.params",
    result: "edit.history.move.result",
    examples: [
      {
        position: 0,
      },
    ],
  },
  "edit.paste": {
    kind: "action",
    params: "edit.paste.params",
    result: "edit.paste.result",
    examples: [
      {},
    ],
    milestone: "M4 C4.1",
  },
  "edit.pasteAsNewElement": {
    kind: "action",
    params: "edit.pasteAsNewElement.params",
    result: "edit.pasteAsNewElement.result",
    examples: [
      {},
    ],
    milestone: "M4 C4.1",
  },
  "edit.pasteSpecial": {
    kind: "action",
    params: "edit.pasteSpecial.params",
    result: "edit.pasteSpecial.result",
    examples: [
      {
        useLast: true,
      },
    ],
    milestone: "M4 C4.1",
  },
  "edit.redo": {
    kind: "action",
    params: "edit.redo.params",
    result: "edit.redo.result",
    examples: [
      {
        steps: 1,
      },
    ],
  },
  "edit.selectAll": {
    kind: "ui",
    params: "edit.selectAll.params",
    result: "edit.selectAll.result",
    examples: [
      {
        context: "staging",
      },
    ],
  },
  "edit.selection.changed": {
    kind: "event",
    params: "edit.selection.changed.params",
    result: "edit.selection.changed.result",
    examples: [
      {},
    ],
    event: true,
  },
  "edit.selection.get": {
    kind: "query",
    params: "edit.selection.get.params",
    result: "edit.selection.get.result",
    examples: [
      {},
    ],
  },
  "edit.selection.set": {
    kind: "ui",
    params: "edit.selection.set.params",
    result: "edit.selection.set.result",
    examples: [
      {
        context: "staging",
        ids: [
          "scene:#1/element:#1",
        ],
      },
    ],
  },
  "edit.undo": {
    kind: "action",
    params: "edit.undo.params",
    result: "edit.undo.result",
    examples: [
      {
        steps: 1,
      },
    ],
  },
  "inspect.contactSheet.render": {
    kind: "query",
    params: "inspect.contactSheet.render.params",
    result: "inspect.contactSheet.render.result",
    examples: [
      {
        frames: {
          from: 1,
          to: 24,
          step: 4,
        },
      },
    ],
    milestone: "M3 C3.3",
  },
  "inspect.coordinates.convert": {
    kind: "query",
    params: "inspect.coordinates.convert.params",
    result: "inspect.coordinates.convert.result",
    examples: [
      {
        from: "field",
        to: "camera",
        points: [
          {
            x: 1,
            y: 1,
          },
        ],
      },
    ],
    milestone: "M3 C3.3",
  },
  "inspect.frame.describe": {
    kind: "query",
    params: "inspect.frame.describe.params",
    result: "inspect.frame.describe.result",
    examples: [
      {},
    ],
    milestone: "M3 C3.3",
  },
  "inspect.hitTest": {
    kind: "query",
    params: "inspect.hitTest.params",
    result: "inspect.hitTest.result",
    examples: [
      {
        space: "camera",
        point: {
          x: 0,
          y: 0,
        },
      },
    ],
    milestone: "M3 C3.3",
  },
  "inspect.screenshot": {
    kind: "query",
    params: "inspect.screenshot.params",
    result: "inspect.screenshot.result",
    examples: [
      {},
    ],
    milestone: "M4 C4.2",
  },
  "inspect.ui.tree": {
    kind: "query",
    params: "inspect.ui.tree.params",
    result: "inspect.ui.tree.result",
    examples: [
      {
        depth: 3,
      },
    ],
    milestone: "M4 C4.2",
  },
  "job.cancel": {
    kind: "ui",
    params: "job.cancel.params",
    result: "job.cancel.result",
    examples: [
      {
        jobId: "job-1",
      },
    ],
  },
  "job.finished": {
    kind: "event",
    params: "job.finished.params",
    result: "job.finished.result",
    examples: [
      {},
    ],
    event: true,
  },
  "job.list": {
    kind: "query",
    params: "job.list.params",
    result: "job.list.result",
    examples: [
      {},
    ],
  },
  "job.progress": {
    kind: "event",
    params: "job.progress.params",
    result: "job.progress.result",
    examples: [
      {},
    ],
    event: true,
  },
  "job.wait": {
    kind: "query",
    params: "job.wait.params",
    result: "job.wait.result",
    examples: [
      {
        jobId: "job-1",
        timeoutMs: 5000,
      },
    ],
  },
  "project.autosaved": {
    kind: "event",
    params: "project.autosaved.params",
    result: "project.autosaved.result",
    examples: [
      {},
    ],
    milestone: "M1 C1.3",
    event: true,
  },
  "project.backup.list": {
    kind: "query",
    params: "project.backup.list.params",
    result: "project.backup.list.result",
    examples: [
      {},
    ],
    milestone: "M1 C1.3",
  },
  "project.backup.open": {
    kind: "action",
    params: "project.backup.open.params",
    result: "project.backup.open.result",
    examples: [
      {
        backupId: "bk_1",
      },
    ],
    milestone: "M1 C1.3",
  },
  "project.close": {
    kind: "action",
    params: "project.close.params",
    result: "project.close.result",
    examples: [
      {},
    ],
    milestone: "M1 C1.3",
  },
  "project.closed": {
    kind: "event",
    params: "project.closed.params",
    result: "project.closed.result",
    examples: [
      {},
    ],
    milestone: "M1 C1.3",
    event: true,
  },
  "project.create": {
    kind: "action",
    params: "project.create.params",
    result: "project.create.result",
    examples: [
      {
        name: "Walk cycle",
        frameRate: 24,
        cameraWidth: 1920,
        cameraHeight: 1080,
      },
    ],
    milestone: "M1 C1.3",
  },
  "project.dirty.changed": {
    kind: "event",
    params: "project.dirty.changed.params",
    result: "project.dirty.changed.result",
    examples: [
      {},
    ],
    event: true,
  },
  "project.info.get": {
    kind: "query",
    params: "project.info.get.params",
    result: "project.info.get.result",
    examples: [
      {},
    ],
    milestone: "M1 C1.3",
  },
  "project.open": {
    kind: "job",
    params: "project.open.params",
    result: "project.open.result",
    examples: [
      {
        path: "/tmp/Short.animpkg",
      },
    ],
    milestone: "M1 C1.3",
  },
  "project.opened": {
    kind: "event",
    params: "project.opened.params",
    result: "project.opened.result",
    examples: [
      {},
    ],
    event: true,
  },
  "project.presets.list": {
    kind: "query",
    params: "project.presets.list.params",
    result: "project.presets.list.result",
    examples: [
      {},
    ],
    milestone: "M1 C1.3",
  },
  "project.properties.get": {
    kind: "query",
    params: "project.properties.get.params",
    result: "project.properties.get.result",
    examples: [
      {},
    ],
    milestone: "M1 C1.3",
  },
  "project.properties.set": {
    kind: "action",
    params: "project.properties.set.params",
    result: "project.properties.set.result",
    examples: [
      {
        frameRate: 25,
      },
    ],
    milestone: "M1 C1.3",
  },
  "project.recent.list": {
    kind: "query",
    params: "project.recent.list.params",
    result: "project.recent.list.result",
    examples: [
      {},
    ],
    milestone: "M1 C1.3",
  },
  "project.recent.remove": {
    kind: "ui",
    params: "project.recent.remove.params",
    result: "project.recent.remove.result",
    examples: [
      {
        all: true,
      },
    ],
    milestone: "M1 C1.3",
  },
  "project.recovery.delete": {
    kind: "ui",
    params: "project.recovery.delete.params",
    result: "project.recovery.delete.result",
    examples: [
      {
        all: true,
      },
    ],
    milestone: "M1 C1.3",
  },
  "project.recovery.list": {
    kind: "query",
    params: "project.recovery.list.params",
    result: "project.recovery.list.result",
    examples: [
      {},
    ],
    milestone: "M1 C1.3",
  },
  "project.recovery.open": {
    kind: "job",
    params: "project.recovery.open.params",
    result: "project.recovery.open.result",
    examples: [
      {
        recoveryId: "rc_1",
      },
    ],
    milestone: "M1 C1.3",
  },
  "project.save": {
    kind: "job",
    params: "project.save.params",
    result: "project.save.result",
    examples: [
      {},
    ],
    milestone: "M1 C1.3",
  },
  "project.saveAs": {
    kind: "job",
    params: "project.saveAs.params",
    result: "project.saveAs.result",
    examples: [
      {
        path: "/tmp/Copy.animpkg",
      },
    ],
    milestone: "M1 C1.3",
  },
  "project.saved": {
    kind: "event",
    params: "project.saved.params",
    result: "project.saved.result",
    examples: [
      {},
    ],
    milestone: "M1 C1.3",
    event: true,
  },
  "window.floatingPanels.set": {
    kind: "toggle",
    params: "window.floatingPanels.set.params",
    result: "window.floatingPanels.set.result",
    examples: [
      {
        visible: false,
      },
    ],
  },
  "window.fullScreen.set": {
    kind: "toggle",
    params: "window.fullScreen.set.params",
    result: "window.fullScreen.set.result",
    examples: [
      {
        enabled: true,
      },
      {
        enabled: false,
      },
    ],
  },
  "window.geometry.get": {
    kind: "query",
    params: "window.geometry.get.params",
    result: "window.geometry.get.result",
    examples: [
      {},
    ],
    milestone: "M4 C4.1",
  },
  "window.geometry.set": {
    kind: "ui",
    params: "window.geometry.set.params",
    result: "window.geometry.set.result",
    examples: [
      {
        width: 1600,
        height: 1000,
      },
    ],
    milestone: "M4 C4.1",
  },
  "window.minimized.set": {
    kind: "toggle",
    params: "window.minimized.set.params",
    result: "window.minimized.set.result",
    examples: [
      {
        minimized: true,
      },
    ],
    milestone: "M4 C4.1",
  },
  "window.panel.list": {
    kind: "query",
    params: "window.panel.list.params",
    result: "window.panel.list.result",
    examples: [
      {},
    ],
  },
  "window.panel.move": {
    kind: "ui",
    params: "window.panel.move.params",
    result: "window.panel.move.result",
    examples: [
      {
        panelId: "history",
        area: "right",
        relativeTo: "properties",
        position: "tab",
      },
    ],
  },
  "window.panel.set": {
    kind: "toggle",
    params: "window.panel.set.params",
    result: "window.panel.set.result",
    examples: [
      {
        panelId: "history",
        visible: true,
      },
      {
        panelId: "colour",
        floating: true,
        geometry: {
          x: 100,
          y: 100,
          width: 320,
          height: 480,
        },
      },
    ],
  },
  "window.raiseAll.run": {
    kind: "ui",
    params: "window.raiseAll.run.params",
    result: "window.raiseAll.run.result",
    examples: [
      {},
    ],
    milestone: "M4 C4.1",
  },
  "window.sidePanelsExtended.set": {
    kind: "toggle",
    params: "window.sidePanelsExtended.set.params",
    result: "window.sidePanelsExtended.set.result",
    examples: [
      {
        enabled: true,
      },
    ],
  },
  "window.toolbar.items.reset": {
    kind: "ui",
    params: "window.toolbar.items.reset.params",
    result: "window.toolbar.items.reset.result",
    examples: [
      {},
    ],
  },
  "window.toolbar.items.set": {
    kind: "ui",
    params: "window.toolbar.items.set.params",
    result: "window.toolbar.items.set.result",
    examples: [
      {
        items: [
          "drawAid.grid.set",
          "separator",
          "playStop",
          "playback.loop.set",
        ],
      },
    ],
  },
  "window.toolbar.list": {
    kind: "query",
    params: "window.toolbar.list.params",
    result: "window.toolbar.list.result",
    examples: [
      {},
    ],
  },
  "window.toolbar.set": {
    kind: "toggle",
    params: "window.toolbar.set.params",
    result: "window.toolbar.set.result",
    examples: [
      {
        toolbarId: "boneTools",
        visible: false,
      },
    ],
  },
  "workspace.changed": {
    kind: "event",
    params: "workspace.changed.params",
    result: "workspace.changed.result",
    examples: [
      {},
    ],
    event: true,
  },
  "workspace.create": {
    kind: "ui",
    params: "workspace.create.params",
    result: "workspace.create.result",
    examples: [
      {
        name: "Compositing",
      },
    ],
  },
  "workspace.delete": {
    kind: "ui",
    params: "workspace.delete.params",
    result: "workspace.delete.result",
    examples: [
      {
        name: "Inking",
      },
    ],
  },
  "workspace.export": {
    kind: "ui",
    params: "workspace.export.params",
    result: "workspace.export.result",
    examples: [
      {
        name: "Drawing",
        path: "/tmp/drawing.ttworkspace",
      },
    ],
    milestone: "M4 C4.1",
  },
  "workspace.get": {
    kind: "query",
    params: "workspace.get.params",
    result: "workspace.get.result",
    examples: [
      {},
      {
        name: "Drawing",
      },
    ],
  },
  "workspace.import": {
    kind: "ui",
    params: "workspace.import.params",
    result: "workspace.import.result",
    examples: [
      {
        path: "/tmp/drawing.ttworkspace",
        onExists: "rename",
      },
    ],
    milestone: "M4 C4.1",
  },
  "workspace.layout.set": {
    kind: "ui",
    params: "workspace.layout.set.params",
    result: "workspace.layout.set.result",
    examples: [
      {
        layout: {
          centralPanel: "cameraView",
          corners: "sides",
          docks: {
            bottom: [
              {
                tabs: [
                  "timeline",
                ],
                current: "timeline",
                size: 200,
              },
            ],
          },
          floating: [],
          toolPalettes: [],
          mainToolbar: {
            visible: true,
            items: [],
          },
          fullScreen: {
            centralPanel: "cameraView",
            corners: "sides",
            docks: {},
            floating: [],
            toolPalettes: [],
            mainToolbar: {
              visible: false,
              items: [],
            },
          },
        },
      },
    ],
  },
  "workspace.list": {
    kind: "query",
    params: "workspace.list.params",
    result: "workspace.list.result",
    examples: [
      {},
    ],
  },
  "workspace.rename": {
    kind: "ui",
    params: "workspace.rename.params",
    result: "workspace.rename.result",
    examples: [
      {
        name: "Staging",
        newName: "Layout",
      },
    ],
  },
  "workspace.reset": {
    kind: "ui",
    params: "workspace.reset.params",
    result: "workspace.reset.result",
    examples: [
      {
        name: "Default",
        select: true,
      },
    ],
  },
  "workspace.save": {
    kind: "ui",
    params: "workspace.save.params",
    result: "workspace.save.result",
    examples: [
      {},
    ],
  },
  "workspace.select": {
    kind: "ui",
    params: "workspace.select.params",
    result: "workspace.select.result",
    examples: [
      {
        name: "Drawing",
      },
    ],
  },
};

export const COMMAND_DEFS: Readonly<Record<string, Schema>> = {
  "envelope.Status": {
    description: "`status` of an envelope.",
    oneOf: [
      {
        description: "The command ran.",
        type: "string",
        const: "ok",
      },
      {
        description: "Confirmation is needed (11 §6.3; the protocol arrives with the MCP server, C1.4).",
        type: "string",
        const: "needs_confirmation",
      },
      {
        description: "The command failed; `error` says why.",
        type: "string",
        const: "error",
      },
    ],
  },
  "envelope.ChangeRecord": {
    description: "One object change of a committed (or dry-run) transaction (10 §2.6, 11 §7.2).",
    type: "object",
    properties: {
      id: {
        description: "Object id (compound for container-local ids).",
        type: "string",
      },
      type: {
        description: "Object type.",
        type: "string",
      },
      op: {
        description: "`created`, `modified` or `deleted`.",
        type: "string",
      },
      rev: {
        description: "The object's `rev` after the transaction (the `documentRev` it would get for a dry run).",
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
      },
      parentId: {
        description: "Parent (created and deleted objects).",
        type: [
          "string",
          "null",
        ],
      },
      index: {
        description: "Position in the parent's collection (created objects).",
        type: [
          "integer",
          "null",
        ],
        format: "uint",
        minimum: 0,
      },
      patch: {
        description: "Own-view RFC 6902 patch with `oldValue` (modified objects).",
        type: [
          "array",
          "null",
        ],
        items: true,
      },
    },
    required: [
      "id",
      "type",
      "op",
      "rev",
    ],
  },
  "envelope.UndoRef": {
    description: "The undo step a transaction created.",
    type: "object",
    properties: {
      stepId: {
        description: "Step id (`u-<n>`).",
        type: "string",
      },
      label: {
        description: "Label.",
        type: "string",
      },
    },
    required: [
      "stepId",
      "label",
    ],
  },
  "envelope.Warning": {
    description: "A warning (11 §6.3, §10): same code space as errors, never aborts.",
    type: "object",
    properties: {
      code: {
        description: "Code.",
        type: "string",
      },
      message: {
        description: "Message.",
        type: "string",
      },
      objectIds: {
        description: "Affected objects.",
        type: "array",
        items: {
          type: "string",
        },
      },
    },
    required: [
      "code",
      "message",
    ],
  },
  "envelope.CommandError": {
    description: "A typed command error in the `{code, message, details, hint?}` wire form (11 §10).",
    type: "object",
    properties: {
      code: {
        description: "Stable dotted code `area.reason`.",
        type: "string",
      },
      message: {
        description: "Human-readable message.",
        type: "string",
      },
      details: {
        description: "Structured details (boxed: the error travels in every `Result` of the command path, so it stays small).",
      },
      hint: {
        description: "How to fix the call, where known.",
        type: [
          "string",
          "null",
        ],
      },
    },
    required: [
      "code",
      "message",
      "details",
    ],
  },
  "envelope.root": {
    title: "Envelope",
    description: "The command result envelope (11 §7.2), also the shape of each batch step.",
    type: "object",
    properties: {
      status: {
        description: "`ok`, `needs_confirmation` or `error`.",
        $ref: "#/$defs/envelope.Status",
      },
      result: {
        description: "Command-specific result (per its result schema).",
      },
      targets: {
        description: "What was actually targeted (P4).",
      },
      changes: {
        description: "Changed objects (capped at [`crate::CHANGES_CAP`]).",
        type: "array",
        items: {
          $ref: "#/$defs/envelope.ChangeRecord",
        },
      },
      changesTruncated: {
        description: "`true` when `changes` was capped; read the rest with `doc.diff` from `changesSince`.",
        type: "boolean",
      },
      changesSince: {
        description: "Revision to pass to `doc.diff` when `changes` was capped.",
        type: [
          "integer",
          "null",
        ],
        maximum: 18446744073709551615n,
        minimum: 0,
      },
      undo: {
        description: "The undo step created.",
        anyOf: [
          {
            $ref: "#/$defs/envelope.UndoRef",
          },
          {
            type: "null",
          },
        ],
      },
      warnings: {
        description: "Warnings.",
        type: "array",
        items: {
          $ref: "#/$defs/envelope.Warning",
        },
      },
      documentRev: {
        description: "`documentRev` after the call (`null` with no project).",
        type: [
          "integer",
          "null",
        ],
        maximum: 18446744073709551615n,
        minimum: 0,
      },
      error: {
        description: "The error of a failed call.",
        anyOf: [
          {
            $ref: "#/$defs/envelope.CommandError",
          },
          {
            type: "null",
          },
        ],
      },
      dryRun: {
        description: "`true` for a dry run.",
        type: "boolean",
      },
      steps: {
        description: "Batch step envelopes.",
        type: [
          "array",
          "null",
        ],
        items: {
          $ref: "#/$defs/envelope.root",
        },
      },
    },
    required: [
      "status",
      "changes",
      "warnings",
    ],
  },
  "agent.activity.export.ActivityFormat": {
    description: "Export format.",
    oneOf: [
      {
        description: "JSON lines.",
        type: "string",
        const: "jsonl",
      },
      {
        description: "A replayable batch.",
        type: "string",
        const: "batch",
      },
    ],
  },
  "agent.activity.export.ExportOnExists": {
    description: "Export conflict policy.",
    oneOf: [
      {
        description: "Fail.",
        type: "string",
        const: "fail",
      },
      {
        description: "Overwrite.",
        type: "string",
        const: "overwrite",
      },
      {
        description: "Pick a new name.",
        type: "string",
        const: "rename",
      },
    ],
  },
  "agent.activity.export.params": {
    title: "ActivityExportParams",
    description: "`agent.activity.export` parameters.",
    type: "object",
    properties: {
      path: {
        description: "Target path.",
        type: "string",
      },
      format: {
        description: "Format.",
        $ref: "#/$defs/agent.activity.export.ActivityFormat",
      },
      onExists: {
        description: "Conflict policy.",
        $ref: "#/$defs/agent.activity.export.ExportOnExists",
      },
    },
    additionalProperties: false,
    required: [
      "path",
    ],
  },
  "agent.activity.export.result": {
    title: "Exported",
    description: "`{path, entries}`.",
    type: "object",
    properties: {
      path: {
        description: "File written.",
        type: "string",
      },
      entries: {
        description: "Entries.",
        type: "integer",
        maximum: 2147483647,
        minimum: 0,
      },
    },
    required: [
      "path",
      "entries",
    ],
  },
  "agent.activity.list.params": {
    title: "ActivityListParams",
    description: "`agent.activity.list` parameters.",
    type: "object",
    properties: {
      sessionId: {
        description: "Session.",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
      since: {
        description: "Since (ISO 8601).",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
      limit: {
        description: "Maximum entries.",
        type: [
          "integer",
          "null",
        ],
        default: null,
        minimum: 1,
        maximum: 10000,
      },
    },
    additionalProperties: false,
  },
  "agent.activity.list.result": {
    title: "Array_of_Map_of_AnyValue",
    type: "array",
    items: {
      type: "object",
      additionalProperties: true,
    },
  },
  "agent.activity.persist.set.params": {
    title: "Enabled",
    description: "`{enabled}`.",
    type: "object",
    properties: {
      enabled: {
        description: "Persist the activity log.",
        type: "boolean",
      },
    },
    additionalProperties: false,
    required: [
      "enabled",
    ],
  },
  "agent.activity.persist.set.result": {
    title: "Enabled",
    description: "`{enabled}`.",
    type: "object",
    properties: {
      enabled: {
        description: "Persist the activity log.",
        type: "boolean",
      },
    },
    additionalProperties: false,
    required: [
      "enabled",
    ],
  },
  "agent.confirmation.requested.params": {
    title: "NoParams",
    description: "No parameters.",
    type: "object",
    additionalProperties: false,
  },
  "agent.confirmation.requested.result": {
    title: "Map_of_AnyValue",
    type: "object",
    additionalProperties: true,
  },
  "agent.confirmation.respond.params": {
    title: "ConfirmParams",
    description: "`agent.confirmation.respond` parameters.",
    type: "object",
    properties: {
      confirmationId: {
        description: "Confirmation.",
        type: "string",
      },
      accept: {
        description: "Accept or decline.",
        type: "boolean",
      },
    },
    additionalProperties: false,
    required: [
      "confirmationId",
      "accept",
    ],
  },
  "agent.confirmation.respond.result": {
    title: "Confirmed",
    description: "`{confirmationId, accepted}`.",
    type: "object",
    properties: {
      confirmationId: {
        description: "Confirmation.",
        type: "string",
      },
      accepted: {
        description: "Accepted.",
        type: "boolean",
      },
    },
    required: [
      "confirmationId",
      "accepted",
    ],
  },
  "agent.policy.defaults.set.Policy": {
    description: "Policy presets (11 §6.1).",
    oneOf: [
      {
        description: "Read only.",
        type: "string",
        const: "observe",
      },
      {
        description: "Asks before consequential actions.",
        type: "string",
        const: "assist",
      },
      {
        description: "Acts within its scopes.",
        type: "string",
        const: "autonomous",
      },
    ],
  },
  "agent.policy.defaults.set.params": {
    title: "PolicyDefaultsParams",
    description: "`agent.policy.defaults.set` parameters.",
    type: "object",
    properties: {
      policy: {
        description: "Default policy.",
        anyOf: [
          {
            $ref: "#/$defs/agent.policy.defaults.set.Policy",
          },
          {
            type: "null",
          },
        ],
      },
      maxJobs: {
        description: "Concurrent jobs per session.",
        type: [
          "integer",
          "null",
        ],
        default: null,
        minimum: 1,
        maximum: 8,
      },
      maxBatchSteps: {
        description: "Steps per batch.",
        type: [
          "integer",
          "null",
        ],
        default: null,
        minimum: 1,
        maximum: 100000,
      },
      maxInlineImagePx: {
        description: "Largest inline image side.",
        type: [
          "integer",
          "null",
        ],
        default: null,
        minimum: 256,
        maximum: 8192,
      },
      maxInlineBytes: {
        description: "Largest inline result.",
        type: [
          "integer",
          "null",
        ],
        default: null,
        minimum: 1,
        maximum: 1073741824,
      },
      askInApp: {
        description: "Ask in the app.",
        type: [
          "boolean",
          "null",
        ],
        default: null,
      },
      letAgentMoveView: {
        description: "Let agents move the view.",
        type: [
          "boolean",
          "null",
        ],
        default: null,
      },
      highlightAgentChanges: {
        description: "Highlight agent changes.",
        type: [
          "boolean",
          "null",
        ],
        default: null,
      },
    },
    additionalProperties: false,
  },
  "agent.policy.defaults.set.result": {
    title: "Map_of_AnyValue",
    type: "object",
    additionalProperties: true,
  },
  "agent.server.set.params": {
    title: "ServerSetParams",
    description: "`agent.server.set` parameters.",
    type: "object",
    properties: {
      enabled: {
        description: "Enable the server.",
        type: "boolean",
      },
      http: {
        description: "Streamable HTTP transport.",
        type: [
          "boolean",
          "null",
        ],
        default: null,
      },
      port: {
        description: "Port.",
        type: [
          "integer",
          "null",
        ],
        default: null,
        minimum: 1024,
        maximum: 65535,
      },
      allowRemote: {
        description: "Accept non-loopback connections.",
        type: [
          "boolean",
          "null",
        ],
        default: null,
      },
    },
    additionalProperties: false,
    required: [
      "enabled",
    ],
  },
  "agent.server.set.result": {
    title: "ServerSetResult",
    description: "`agent.server.set` result.",
    type: "object",
    properties: {
      enabled: {
        description: "Enabled.",
        type: "boolean",
      },
      endpoint: {
        description: "Endpoint URL.",
        type: [
          "string",
          "null",
        ],
      },
      discoveryFile: {
        description: "Discovery file path.",
        type: [
          "string",
          "null",
        ],
      },
    },
    required: [
      "enabled",
    ],
  },
  "agent.server.status.params": {
    title: "NoParams",
    description: "No parameters.",
    type: "object",
    additionalProperties: false,
  },
  "agent.server.status.result": {
    title: "Map_of_AnyValue",
    type: "object",
    additionalProperties: true,
  },
  "agent.session.connected.params": {
    title: "NoParams",
    description: "No parameters.",
    type: "object",
    additionalProperties: false,
  },
  "agent.session.connected.result": {
    title: "Map_of_AnyValue",
    type: "object",
    additionalProperties: true,
  },
  "agent.session.disconnect.params": {
    title: "SessionParams",
    description: "`{sessionId}` parameters.",
    type: "object",
    properties: {
      sessionId: {
        description: "Session.",
        type: "string",
      },
    },
    additionalProperties: false,
    required: [
      "sessionId",
    ],
  },
  "agent.session.disconnect.result": {
    title: "Empty",
    description: "An empty result object.",
    type: "object",
  },
  "agent.session.disconnected.params": {
    title: "NoParams",
    description: "No parameters.",
    type: "object",
    additionalProperties: false,
  },
  "agent.session.disconnected.result": {
    title: "Map_of_AnyValue",
    type: "object",
    additionalProperties: true,
  },
  "agent.session.grant.Policy": {
    description: "Policy presets (11 §6.1).",
    oneOf: [
      {
        description: "Read only.",
        type: "string",
        const: "observe",
      },
      {
        description: "Asks before consequential actions.",
        type: "string",
        const: "assist",
      },
      {
        description: "Acts within its scopes.",
        type: "string",
        const: "autonomous",
      },
    ],
  },
  "agent.session.grant.params": {
    title: "GrantParams",
    description: "`agent.session.grant` parameters.",
    type: "object",
    properties: {
      sessionId: {
        description: "Session.",
        type: "string",
      },
      policy: {
        description: "Policy preset.",
        anyOf: [
          {
            $ref: "#/$defs/agent.session.grant.Policy",
          },
          {
            type: "null",
          },
        ],
      },
      scopes: {
        description: "Scopes.",
        type: [
          "array",
          "null",
        ],
        items: {
          type: "string",
        },
        default: null,
      },
      roots: {
        description: "File-system roots.",
        type: [
          "array",
          "null",
        ],
        items: {
          type: "string",
        },
        default: null,
      },
    },
    additionalProperties: false,
    required: [
      "sessionId",
    ],
  },
  "agent.session.grant.result": {
    title: "Map_of_AnyValue",
    type: "object",
    additionalProperties: true,
  },
  "agent.session.pause.params": {
    title: "OptionalSession",
    description: "`{sessionId?}` parameters.",
    type: "object",
    properties: {
      sessionId: {
        description: "Session (default: all).",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
    },
    additionalProperties: false,
  },
  "agent.session.pause.result": {
    title: "Paused",
    description: "`{paused}`.",
    type: "object",
    properties: {
      paused: {
        description: "Paused sessions.",
        type: "array",
        items: {
          type: "string",
        },
      },
    },
    required: [
      "paused",
    ],
  },
  "agent.session.resume.params": {
    title: "OptionalSession",
    description: "`{sessionId?}` parameters.",
    type: "object",
    properties: {
      sessionId: {
        description: "Session (default: all).",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
    },
    additionalProperties: false,
  },
  "agent.session.resume.result": {
    title: "Resumed",
    description: "`{resumed}`.",
    type: "object",
    properties: {
      resumed: {
        description: "Resumed sessions.",
        type: "array",
        items: {
          type: "string",
        },
      },
    },
    required: [
      "resumed",
    ],
  },
  "agent.token.regenerate.params": {
    title: "NoParams",
    description: "No parameters.",
    type: "object",
    additionalProperties: false,
  },
  "agent.token.regenerate.result": {
    title: "Discovery",
    description: "`{discoveryFile}`.",
    type: "object",
    properties: {
      discoveryFile: {
        description: "Discovery file path.",
        type: "string",
      },
    },
    required: [
      "discoveryFile",
    ],
  },
  "agent.typedTools.set.params": {
    title: "TypedToolsParams",
    description: "`agent.typedTools.set` parameters.",
    type: "object",
    properties: {
      enabled: {
        description: "Expose one MCP tool per command.",
        type: "boolean",
      },
      namespaces: {
        description: "Limit to namespaces.",
        type: [
          "array",
          "null",
        ],
        items: {
          type: "string",
        },
        default: null,
      },
    },
    additionalProperties: false,
    required: [
      "enabled",
    ],
  },
  "agent.typedTools.set.result": {
    title: "ToolCount",
    description: "`{toolCount}`.",
    type: "object",
    properties: {
      toolCount: {
        description: "Tools exposed.",
        type: "integer",
        maximum: 2147483647,
        minimum: 0,
      },
    },
    required: [
      "toolCount",
    ],
  },
  "app.context.set.params": {
    title: "ContextSetParams",
    description: "`app.context.set` parameters.",
    type: "object",
    properties: {
      sceneId: {
        description: "Scene to make current (id or path).",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
      frame: {
        description: "Frame (1-based).",
        type: [
          "integer",
          "null",
        ],
        default: null,
        minimum: 1,
        maximum: 100000,
      },
      elementId: {
        description: "Element to make current (must belong to the scene).",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
      drawingId: {
        description: "Drawing to make current (must belong to the element's drawing bank).",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
    },
    additionalProperties: false,
  },
  "app.context.set.ProjectRef": {
    description: "The open project in `AppState`.",
    type: "object",
    properties: {
      id: {
        description: "Project id.",
        type: "string",
      },
      name: {
        description: "Name.",
        type: "string",
      },
      path: {
        description: "File path (`null` for an unsaved project).",
        type: [
          "string",
          "null",
        ],
      },
      dirty: {
        description: "Modified since the last save.",
        type: "boolean",
      },
      globalLibraryDirty: {
        description: "The global library has unsaved changes (08; always `false` until M9).",
        type: "boolean",
      },
      readOnly: {
        description: "Opened read-only.",
        type: "boolean",
      },
    },
    required: [
      "id",
      "name",
      "dirty",
      "globalLibraryDirty",
      "readOnly",
    ],
  },
  "app.context.set.SceneRef": {
    description: "`{id, name, frameCount}` of a scene.",
    type: "object",
    properties: {
      id: {
        description: "Scene id.",
        type: "string",
      },
      name: {
        description: "Name.",
        type: "string",
      },
      frameCount: {
        description: "Scene length (derived, 10 §4.3).",
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
      },
    },
    required: [
      "id",
      "name",
      "frameCount",
    ],
  },
  "app.context.set.ElementRef": {
    description: "`{id, name, type}` of an element.",
    type: "object",
    properties: {
      id: {
        description: "Element id.",
        type: "string",
      },
      name: {
        description: "Name.",
        type: "string",
      },
      type: {
        description: "Element type (01 §12.4 reports `elementType` as `type`).",
        type: "string",
      },
    },
    required: [
      "id",
      "name",
      "type",
    ],
  },
  "app.context.set.DrawingRef": {
    description: "`{id, name}` of a drawing.",
    type: "object",
    properties: {
      id: {
        description: "Drawing id.",
        type: "string",
      },
      name: {
        description: "Name.",
        type: "string",
      },
    },
    required: [
      "id",
      "name",
    ],
  },
  "app.context.set.Mode": {
    description: "The working mode (01 §4.4).",
    oneOf: [
      {
        description: "Drawing mode.",
        type: "string",
        const: "drawing",
      },
      {
        description: "Staging (animation) mode.",
        type: "string",
        const: "staging",
      },
    ],
  },
  "app.context.set.ActiveView": {
    description: "The active view.",
    type: "object",
    properties: {
      viewId: {
        description: "View id (the kernel has one view per view panel; ids equal panel ids until M4).",
        type: "string",
      },
      panelId: {
        description: "Panel id.",
        type: "string",
      },
      type: {
        description: "`camera`, `drawing`, `top`, `side`.",
        type: "string",
      },
    },
    required: [
      "viewId",
      "panelId",
      "type",
    ],
  },
  "app.context.set.ActiveTool": {
    description: "The active tool (M4 C4.2 selects tools; `null` until then).",
    type: "object",
    properties: {
      toolId: {
        description: "Tool id.",
        type: "string",
      },
    },
    required: [
      "toolId",
    ],
  },
  "app.context.set.SelectionState": {
    description: "The selection in `AppState`.",
    type: "object",
    properties: {
      context: {
        description: "Selection context.",
        type: "string",
      },
      ids: {
        description: "Selected ids.",
        type: "array",
        items: {
          type: "string",
        },
      },
      kinds: {
        description: "Object types present in the selection.",
        type: "array",
        items: {
          type: "string",
        },
      },
    },
    required: [
      "context",
      "ids",
      "kinds",
    ],
  },
  "app.context.set.WorkspaceState": {
    description: "The workspace in `AppState`.",
    type: "object",
    properties: {
      name: {
        description: "Current workspace.",
        type: "string",
      },
      modified: {
        description: "The arrangement differs from the stored one.",
        type: "boolean",
      },
    },
    required: [
      "name",
      "modified",
    ],
  },
  "app.context.set.PlaybackState": {
    description: "Playback in `AppState` (M5 drives it; the kernel reports stopped).",
    type: "object",
    properties: {
      playing: {
        description: "Playing.",
        type: "boolean",
      },
    },
    required: [
      "playing",
    ],
  },
  "app.context.set.Modal": {
    description: "A modal GUI state (never set by the kernel: 11 P5).",
    type: "object",
    properties: {
      kind: {
        description: "`dialog`, `gesture` or `progress`.",
        type: "string",
      },
      title: {
        description: "Title.",
        type: "string",
      },
    },
    required: [
      "kind",
      "title",
    ],
  },
  "app.context.set.AgentSessionState": {
    description: "An agent session in `AppState` (C1.4 fills the list).",
    type: "object",
    properties: {
      sessionId: {
        description: "Session id.",
        type: "string",
      },
      name: {
        description: "Client display name.",
        type: "string",
      },
      acting: {
        description: "Currently running a command.",
        type: "boolean",
      },
    },
    required: [
      "sessionId",
      "name",
      "acting",
    ],
  },
  "app.context.set.result": {
    title: "AppState",
    description: "`AppState` (01 §12.4).",
    type: "object",
    properties: {
      project: {
        description: "Open project or `null`.",
        anyOf: [
          {
            $ref: "#/$defs/app.context.set.ProjectRef",
          },
          {
            type: "null",
          },
        ],
      },
      scene: {
        description: "Current scene.",
        anyOf: [
          {
            $ref: "#/$defs/app.context.set.SceneRef",
          },
          {
            type: "null",
          },
        ],
      },
      frame: {
        description: "Current frame (`null` without a scene).",
        type: [
          "integer",
          "null",
        ],
        maximum: 2147483647,
        minimum: 0,
      },
      element: {
        description: "Current element.",
        anyOf: [
          {
            $ref: "#/$defs/app.context.set.ElementRef",
          },
          {
            type: "null",
          },
        ],
      },
      drawing: {
        description: "Current drawing.",
        anyOf: [
          {
            $ref: "#/$defs/app.context.set.DrawingRef",
          },
          {
            type: "null",
          },
        ],
      },
      mode: {
        description: "Working mode (`null` without a project).",
        anyOf: [
          {
            $ref: "#/$defs/app.context.set.Mode",
          },
          {
            type: "null",
          },
        ],
      },
      activeView: {
        description: "Active view.",
        anyOf: [
          {
            $ref: "#/$defs/app.context.set.ActiveView",
          },
          {
            type: "null",
          },
        ],
      },
      focusedPanel: {
        description: "Focused panel.",
        type: [
          "string",
          "null",
        ],
      },
      activeTool: {
        description: "Active tool.",
        anyOf: [
          {
            $ref: "#/$defs/app.context.set.ActiveTool",
          },
          {
            type: "null",
          },
        ],
      },
      selection: {
        description: "Selection.",
        $ref: "#/$defs/app.context.set.SelectionState",
      },
      workspace: {
        description: "Workspace.",
        $ref: "#/$defs/app.context.set.WorkspaceState",
      },
      playback: {
        description: "Playback.",
        $ref: "#/$defs/app.context.set.PlaybackState",
      },
      startScreenVisible: {
        description: "Start screen shown (no project).",
        type: "boolean",
      },
      modal: {
        description: "Modal state (always `null` in the kernel).",
        anyOf: [
          {
            $ref: "#/$defs/app.context.set.Modal",
          },
          {
            type: "null",
          },
        ],
      },
      agentSessions: {
        description: "Connected agent sessions.",
        type: "array",
        items: {
          $ref: "#/$defs/app.context.set.AgentSessionState",
        },
      },
      fullScreen: {
        description: "Full-screen presentation (01 §5.6; explicit `enabled` state, readiness bullet 4).",
        type: "boolean",
      },
    },
    required: [
      "selection",
      "workspace",
      "playback",
      "startScreenVisible",
      "agentSessions",
      "fullScreen",
    ],
  },
  "app.currentElement.move.Direction": {
    description: "Previous / next.",
    oneOf: [
      {
        description: "Backwards.",
        type: "string",
        const: "previous",
      },
      {
        description: "Forwards.",
        type: "string",
        const: "next",
      },
    ],
  },
  "app.currentElement.move.ElementOrder": {
    description: "Element order for `app.currentElement.move`.",
    oneOf: [
      {
        description: "Exposure-sheet order when the Exposure Sheet has focus, else Timeline order.",
        type: "string",
        const: "auto",
      },
      {
        description: "Timeline order (depth-first, parents first).",
        type: "string",
        const: "timeline",
      },
      {
        description: "Exposure Sheet column order (cell-based elements).",
        type: "string",
        const: "xsheet",
      },
    ],
  },
  "app.currentElement.move.params": {
    title: "CurrentElementMoveParams",
    description: "`app.currentElement.move` parameters.",
    type: "object",
    properties: {
      direction: {
        description: "Previous or next.",
        $ref: "#/$defs/app.currentElement.move.Direction",
      },
      order: {
        description: "Which order to follow.",
        $ref: "#/$defs/app.currentElement.move.ElementOrder",
      },
    },
    additionalProperties: false,
    required: [
      "direction",
    ],
  },
  "app.currentElement.move.result": {
    title: "ElementIdResult",
    description: "`{elementId}`.",
    type: "object",
    properties: {
      elementId: {
        description: "The current element (`null` when the scene has none).",
        type: [
          "string",
          "null",
        ],
      },
    },
  },
  "app.currentElement.set.params": {
    title: "CurrentElementSetParams",
    description: "`app.currentElement.set` parameters.",
    type: "object",
    properties: {
      elementId: {
        description: "The element (id or path); its scene becomes current.",
        type: "string",
      },
    },
    additionalProperties: false,
    required: [
      "elementId",
    ],
  },
  "app.currentElement.set.result": {
    title: "ElementIdResult",
    description: "`{elementId}`.",
    type: "object",
    properties: {
      elementId: {
        description: "The current element (`null` when the scene has none).",
        type: [
          "string",
          "null",
        ],
      },
    },
  },
  "app.frame.move.Direction": {
    description: "Previous / next.",
    oneOf: [
      {
        description: "Backwards.",
        type: "string",
        const: "previous",
      },
      {
        description: "Forwards.",
        type: "string",
        const: "next",
      },
    ],
  },
  "app.frame.move.FrameUnit": {
    description: "Step unit for `app.frame.move`.",
    oneOf: [
      {
        description: "One frame.",
        type: "string",
        const: "frame",
      },
      {
        description: "To the next/previous frame where the current element exposes another drawing.",
        type: "string",
        const: "drawing",
      },
    ],
  },
  "app.frame.move.params": {
    title: "FrameMoveParams",
    description: "`app.frame.move` parameters.",
    type: "object",
    properties: {
      direction: {
        description: "Previous or next.",
        $ref: "#/$defs/app.frame.move.Direction",
      },
      unit: {
        description: "Frames or drawings.",
        $ref: "#/$defs/app.frame.move.FrameUnit",
      },
      count: {
        description: "How many steps.",
        type: "integer",
        default: 1,
        minimum: 1,
        maximum: 10000,
      },
    },
    additionalProperties: false,
    required: [
      "direction",
    ],
  },
  "app.frame.move.result": {
    title: "FrameResult",
    description: "`{frame}`.",
    type: "object",
    properties: {
      frame: {
        description: "The current frame.",
        type: "integer",
        maximum: 2147483647,
        minimum: 0,
      },
    },
    required: [
      "frame",
    ],
  },
  "app.frame.set.params": {
    title: "FrameSetParams",
    description: "`app.frame.set` parameters.",
    type: "object",
    properties: {
      frame: {
        description: "Frame, 1 to the scene length.",
        type: "integer",
        maximum: 100000,
        minimum: 1,
      },
    },
    additionalProperties: false,
    required: [
      "frame",
    ],
  },
  "app.frame.set.result": {
    title: "FrameResult",
    description: "`{frame}`.",
    type: "object",
    properties: {
      frame: {
        description: "The current frame.",
        type: "integer",
        maximum: 2147483647,
        minimum: 0,
      },
    },
    required: [
      "frame",
    ],
  },
  "app.info.get.params": {
    title: "NoParams",
    description: "No parameters.",
    type: "object",
    additionalProperties: false,
  },
  "app.info.get.result": {
    title: "InfoResult",
    description: "`app.info.get` result.",
    type: "object",
    properties: {
      name: {
        description: "Application name.",
        type: "string",
      },
      version: {
        description: "Version.",
        type: "string",
      },
      platform: {
        description: "Platform.",
        type: "string",
      },
      locale: {
        description: "UI locale.",
        type: "string",
      },
      languages: {
        description: "Available UI languages.",
        type: "array",
        items: {
          type: "string",
        },
      },
      formatVersion: {
        description: "Project format version.",
        type: "string",
      },
    },
    required: [
      "name",
      "version",
      "platform",
      "locale",
      "languages",
      "formatVersion",
    ],
  },
  "app.mode.set.ModeParam": {
    description: "Working mode parameter.",
    oneOf: [
      {
        description: "Drawing mode.",
        type: "string",
        const: "drawing",
      },
      {
        description: "Staging mode.",
        type: "string",
        const: "staging",
      },
    ],
  },
  "app.mode.set.params": {
    title: "ModeSetParams",
    description: "`app.mode.set` parameters.",
    type: "object",
    properties: {
      mode: {
        description: "Drawing or staging.",
        $ref: "#/$defs/app.mode.set.ModeParam",
      },
      viewId: {
        description: "The view to switch (a view panel id); default the central view.",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
    },
    additionalProperties: false,
    required: [
      "mode",
    ],
  },
  "app.mode.set.Mode": {
    description: "The working mode (01 §4.4).",
    oneOf: [
      {
        description: "Drawing mode.",
        type: "string",
        const: "drawing",
      },
      {
        description: "Staging (animation) mode.",
        type: "string",
        const: "staging",
      },
    ],
  },
  "app.mode.set.result": {
    title: "ModeResult",
    description: "`{mode, viewId}`.",
    type: "object",
    properties: {
      mode: {
        description: "The mode.",
        $ref: "#/$defs/app.mode.set.Mode",
      },
      viewId: {
        description: "The view.",
        type: "string",
      },
    },
    required: [
      "mode",
      "viewId",
    ],
  },
  "app.quit.OnUnsaved": {
    description: "What to do with unsaved changes (01 §11.2 rule 3).",
    oneOf: [
      {
        description: "Fail with `project.unsavedChanges` when something is modified.",
        type: "string",
        const: "fail",
      },
      {
        description: "Save first.",
        type: "string",
        const: "save",
      },
      {
        description: "Discard the changes.",
        type: "string",
        const: "discard",
      },
    ],
  },
  "app.quit.params": {
    title: "QuitParams",
    description: "`app.quit` parameters.",
    type: "object",
    properties: {
      onUnsaved: {
        description: "Unsaved-changes policy.",
        $ref: "#/$defs/app.quit.OnUnsaved",
      },
    },
    additionalProperties: false,
  },
  "app.quit.result": {
    title: "QuitResult",
    description: "`{quitting}`.",
    type: "object",
    properties: {
      quitting: {
        description: "The application quits.",
        type: "boolean",
      },
    },
    required: [
      "quitting",
    ],
  },
  "app.state.changed.params": {
    title: "NoParams",
    description: "No parameters.",
    type: "object",
    additionalProperties: false,
  },
  "app.state.changed.AppState": {
    description: "`AppState` (01 §12.4).",
    type: "object",
    properties: {
      project: {
        description: "Open project or `null`.",
        anyOf: [
          {
            $ref: "#/$defs/app.state.changed.ProjectRef",
          },
          {
            type: "null",
          },
        ],
      },
      scene: {
        description: "Current scene.",
        anyOf: [
          {
            $ref: "#/$defs/app.state.changed.SceneRef",
          },
          {
            type: "null",
          },
        ],
      },
      frame: {
        description: "Current frame (`null` without a scene).",
        type: [
          "integer",
          "null",
        ],
        maximum: 2147483647,
        minimum: 0,
      },
      element: {
        description: "Current element.",
        anyOf: [
          {
            $ref: "#/$defs/app.state.changed.ElementRef",
          },
          {
            type: "null",
          },
        ],
      },
      drawing: {
        description: "Current drawing.",
        anyOf: [
          {
            $ref: "#/$defs/app.state.changed.DrawingRef",
          },
          {
            type: "null",
          },
        ],
      },
      mode: {
        description: "Working mode (`null` without a project).",
        anyOf: [
          {
            $ref: "#/$defs/app.state.changed.Mode",
          },
          {
            type: "null",
          },
        ],
      },
      activeView: {
        description: "Active view.",
        anyOf: [
          {
            $ref: "#/$defs/app.state.changed.ActiveView",
          },
          {
            type: "null",
          },
        ],
      },
      focusedPanel: {
        description: "Focused panel.",
        type: [
          "string",
          "null",
        ],
      },
      activeTool: {
        description: "Active tool.",
        anyOf: [
          {
            $ref: "#/$defs/app.state.changed.ActiveTool",
          },
          {
            type: "null",
          },
        ],
      },
      selection: {
        description: "Selection.",
        $ref: "#/$defs/app.state.changed.SelectionState",
      },
      workspace: {
        description: "Workspace.",
        $ref: "#/$defs/app.state.changed.WorkspaceState",
      },
      playback: {
        description: "Playback.",
        $ref: "#/$defs/app.state.changed.PlaybackState",
      },
      startScreenVisible: {
        description: "Start screen shown (no project).",
        type: "boolean",
      },
      modal: {
        description: "Modal state (always `null` in the kernel).",
        anyOf: [
          {
            $ref: "#/$defs/app.state.changed.Modal",
          },
          {
            type: "null",
          },
        ],
      },
      agentSessions: {
        description: "Connected agent sessions.",
        type: "array",
        items: {
          $ref: "#/$defs/app.state.changed.AgentSessionState",
        },
      },
      fullScreen: {
        description: "Full-screen presentation (01 §5.6; explicit `enabled` state, readiness bullet 4).",
        type: "boolean",
      },
    },
    required: [
      "selection",
      "workspace",
      "playback",
      "startScreenVisible",
      "agentSessions",
      "fullScreen",
    ],
  },
  "app.state.changed.ProjectRef": {
    description: "The open project in `AppState`.",
    type: "object",
    properties: {
      id: {
        description: "Project id.",
        type: "string",
      },
      name: {
        description: "Name.",
        type: "string",
      },
      path: {
        description: "File path (`null` for an unsaved project).",
        type: [
          "string",
          "null",
        ],
      },
      dirty: {
        description: "Modified since the last save.",
        type: "boolean",
      },
      globalLibraryDirty: {
        description: "The global library has unsaved changes (08; always `false` until M9).",
        type: "boolean",
      },
      readOnly: {
        description: "Opened read-only.",
        type: "boolean",
      },
    },
    required: [
      "id",
      "name",
      "dirty",
      "globalLibraryDirty",
      "readOnly",
    ],
  },
  "app.state.changed.SceneRef": {
    description: "`{id, name, frameCount}` of a scene.",
    type: "object",
    properties: {
      id: {
        description: "Scene id.",
        type: "string",
      },
      name: {
        description: "Name.",
        type: "string",
      },
      frameCount: {
        description: "Scene length (derived, 10 §4.3).",
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
      },
    },
    required: [
      "id",
      "name",
      "frameCount",
    ],
  },
  "app.state.changed.ElementRef": {
    description: "`{id, name, type}` of an element.",
    type: "object",
    properties: {
      id: {
        description: "Element id.",
        type: "string",
      },
      name: {
        description: "Name.",
        type: "string",
      },
      type: {
        description: "Element type (01 §12.4 reports `elementType` as `type`).",
        type: "string",
      },
    },
    required: [
      "id",
      "name",
      "type",
    ],
  },
  "app.state.changed.DrawingRef": {
    description: "`{id, name}` of a drawing.",
    type: "object",
    properties: {
      id: {
        description: "Drawing id.",
        type: "string",
      },
      name: {
        description: "Name.",
        type: "string",
      },
    },
    required: [
      "id",
      "name",
    ],
  },
  "app.state.changed.Mode": {
    description: "The working mode (01 §4.4).",
    oneOf: [
      {
        description: "Drawing mode.",
        type: "string",
        const: "drawing",
      },
      {
        description: "Staging (animation) mode.",
        type: "string",
        const: "staging",
      },
    ],
  },
  "app.state.changed.ActiveView": {
    description: "The active view.",
    type: "object",
    properties: {
      viewId: {
        description: "View id (the kernel has one view per view panel; ids equal panel ids until M4).",
        type: "string",
      },
      panelId: {
        description: "Panel id.",
        type: "string",
      },
      type: {
        description: "`camera`, `drawing`, `top`, `side`.",
        type: "string",
      },
    },
    required: [
      "viewId",
      "panelId",
      "type",
    ],
  },
  "app.state.changed.ActiveTool": {
    description: "The active tool (M4 C4.2 selects tools; `null` until then).",
    type: "object",
    properties: {
      toolId: {
        description: "Tool id.",
        type: "string",
      },
    },
    required: [
      "toolId",
    ],
  },
  "app.state.changed.SelectionState": {
    description: "The selection in `AppState`.",
    type: "object",
    properties: {
      context: {
        description: "Selection context.",
        type: "string",
      },
      ids: {
        description: "Selected ids.",
        type: "array",
        items: {
          type: "string",
        },
      },
      kinds: {
        description: "Object types present in the selection.",
        type: "array",
        items: {
          type: "string",
        },
      },
    },
    required: [
      "context",
      "ids",
      "kinds",
    ],
  },
  "app.state.changed.WorkspaceState": {
    description: "The workspace in `AppState`.",
    type: "object",
    properties: {
      name: {
        description: "Current workspace.",
        type: "string",
      },
      modified: {
        description: "The arrangement differs from the stored one.",
        type: "boolean",
      },
    },
    required: [
      "name",
      "modified",
    ],
  },
  "app.state.changed.PlaybackState": {
    description: "Playback in `AppState` (M5 drives it; the kernel reports stopped).",
    type: "object",
    properties: {
      playing: {
        description: "Playing.",
        type: "boolean",
      },
    },
    required: [
      "playing",
    ],
  },
  "app.state.changed.Modal": {
    description: "A modal GUI state (never set by the kernel: 11 P5).",
    type: "object",
    properties: {
      kind: {
        description: "`dialog`, `gesture` or `progress`.",
        type: "string",
      },
      title: {
        description: "Title.",
        type: "string",
      },
    },
    required: [
      "kind",
      "title",
    ],
  },
  "app.state.changed.AgentSessionState": {
    description: "An agent session in `AppState` (C1.4 fills the list).",
    type: "object",
    properties: {
      sessionId: {
        description: "Session id.",
        type: "string",
      },
      name: {
        description: "Client display name.",
        type: "string",
      },
      acting: {
        description: "Currently running a command.",
        type: "boolean",
      },
    },
    required: [
      "sessionId",
      "name",
      "acting",
    ],
  },
  "app.state.changed.result": {
    title: "StateChanged",
    description: "`app.state.changed` payload.",
    type: "object",
    properties: {
      changed: {
        description: "Top-level `AppState` fields that changed.",
        type: "array",
        items: {
          type: "string",
        },
      },
      state: {
        description: "The new state.",
        $ref: "#/$defs/app.state.changed.AppState",
      },
    },
    required: [
      "changed",
      "state",
    ],
  },
  "app.state.get.params": {
    title: "NoParams",
    description: "No parameters.",
    type: "object",
    additionalProperties: false,
  },
  "app.state.get.ProjectRef": {
    description: "The open project in `AppState`.",
    type: "object",
    properties: {
      id: {
        description: "Project id.",
        type: "string",
      },
      name: {
        description: "Name.",
        type: "string",
      },
      path: {
        description: "File path (`null` for an unsaved project).",
        type: [
          "string",
          "null",
        ],
      },
      dirty: {
        description: "Modified since the last save.",
        type: "boolean",
      },
      globalLibraryDirty: {
        description: "The global library has unsaved changes (08; always `false` until M9).",
        type: "boolean",
      },
      readOnly: {
        description: "Opened read-only.",
        type: "boolean",
      },
    },
    required: [
      "id",
      "name",
      "dirty",
      "globalLibraryDirty",
      "readOnly",
    ],
  },
  "app.state.get.SceneRef": {
    description: "`{id, name, frameCount}` of a scene.",
    type: "object",
    properties: {
      id: {
        description: "Scene id.",
        type: "string",
      },
      name: {
        description: "Name.",
        type: "string",
      },
      frameCount: {
        description: "Scene length (derived, 10 §4.3).",
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
      },
    },
    required: [
      "id",
      "name",
      "frameCount",
    ],
  },
  "app.state.get.ElementRef": {
    description: "`{id, name, type}` of an element.",
    type: "object",
    properties: {
      id: {
        description: "Element id.",
        type: "string",
      },
      name: {
        description: "Name.",
        type: "string",
      },
      type: {
        description: "Element type (01 §12.4 reports `elementType` as `type`).",
        type: "string",
      },
    },
    required: [
      "id",
      "name",
      "type",
    ],
  },
  "app.state.get.DrawingRef": {
    description: "`{id, name}` of a drawing.",
    type: "object",
    properties: {
      id: {
        description: "Drawing id.",
        type: "string",
      },
      name: {
        description: "Name.",
        type: "string",
      },
    },
    required: [
      "id",
      "name",
    ],
  },
  "app.state.get.Mode": {
    description: "The working mode (01 §4.4).",
    oneOf: [
      {
        description: "Drawing mode.",
        type: "string",
        const: "drawing",
      },
      {
        description: "Staging (animation) mode.",
        type: "string",
        const: "staging",
      },
    ],
  },
  "app.state.get.ActiveView": {
    description: "The active view.",
    type: "object",
    properties: {
      viewId: {
        description: "View id (the kernel has one view per view panel; ids equal panel ids until M4).",
        type: "string",
      },
      panelId: {
        description: "Panel id.",
        type: "string",
      },
      type: {
        description: "`camera`, `drawing`, `top`, `side`.",
        type: "string",
      },
    },
    required: [
      "viewId",
      "panelId",
      "type",
    ],
  },
  "app.state.get.ActiveTool": {
    description: "The active tool (M4 C4.2 selects tools; `null` until then).",
    type: "object",
    properties: {
      toolId: {
        description: "Tool id.",
        type: "string",
      },
    },
    required: [
      "toolId",
    ],
  },
  "app.state.get.SelectionState": {
    description: "The selection in `AppState`.",
    type: "object",
    properties: {
      context: {
        description: "Selection context.",
        type: "string",
      },
      ids: {
        description: "Selected ids.",
        type: "array",
        items: {
          type: "string",
        },
      },
      kinds: {
        description: "Object types present in the selection.",
        type: "array",
        items: {
          type: "string",
        },
      },
    },
    required: [
      "context",
      "ids",
      "kinds",
    ],
  },
  "app.state.get.WorkspaceState": {
    description: "The workspace in `AppState`.",
    type: "object",
    properties: {
      name: {
        description: "Current workspace.",
        type: "string",
      },
      modified: {
        description: "The arrangement differs from the stored one.",
        type: "boolean",
      },
    },
    required: [
      "name",
      "modified",
    ],
  },
  "app.state.get.PlaybackState": {
    description: "Playback in `AppState` (M5 drives it; the kernel reports stopped).",
    type: "object",
    properties: {
      playing: {
        description: "Playing.",
        type: "boolean",
      },
    },
    required: [
      "playing",
    ],
  },
  "app.state.get.Modal": {
    description: "A modal GUI state (never set by the kernel: 11 P5).",
    type: "object",
    properties: {
      kind: {
        description: "`dialog`, `gesture` or `progress`.",
        type: "string",
      },
      title: {
        description: "Title.",
        type: "string",
      },
    },
    required: [
      "kind",
      "title",
    ],
  },
  "app.state.get.AgentSessionState": {
    description: "An agent session in `AppState` (C1.4 fills the list).",
    type: "object",
    properties: {
      sessionId: {
        description: "Session id.",
        type: "string",
      },
      name: {
        description: "Client display name.",
        type: "string",
      },
      acting: {
        description: "Currently running a command.",
        type: "boolean",
      },
    },
    required: [
      "sessionId",
      "name",
      "acting",
    ],
  },
  "app.state.get.result": {
    title: "AppState",
    description: "`AppState` (01 §12.4).",
    type: "object",
    properties: {
      project: {
        description: "Open project or `null`.",
        anyOf: [
          {
            $ref: "#/$defs/app.state.get.ProjectRef",
          },
          {
            type: "null",
          },
        ],
      },
      scene: {
        description: "Current scene.",
        anyOf: [
          {
            $ref: "#/$defs/app.state.get.SceneRef",
          },
          {
            type: "null",
          },
        ],
      },
      frame: {
        description: "Current frame (`null` without a scene).",
        type: [
          "integer",
          "null",
        ],
        maximum: 2147483647,
        minimum: 0,
      },
      element: {
        description: "Current element.",
        anyOf: [
          {
            $ref: "#/$defs/app.state.get.ElementRef",
          },
          {
            type: "null",
          },
        ],
      },
      drawing: {
        description: "Current drawing.",
        anyOf: [
          {
            $ref: "#/$defs/app.state.get.DrawingRef",
          },
          {
            type: "null",
          },
        ],
      },
      mode: {
        description: "Working mode (`null` without a project).",
        anyOf: [
          {
            $ref: "#/$defs/app.state.get.Mode",
          },
          {
            type: "null",
          },
        ],
      },
      activeView: {
        description: "Active view.",
        anyOf: [
          {
            $ref: "#/$defs/app.state.get.ActiveView",
          },
          {
            type: "null",
          },
        ],
      },
      focusedPanel: {
        description: "Focused panel.",
        type: [
          "string",
          "null",
        ],
      },
      activeTool: {
        description: "Active tool.",
        anyOf: [
          {
            $ref: "#/$defs/app.state.get.ActiveTool",
          },
          {
            type: "null",
          },
        ],
      },
      selection: {
        description: "Selection.",
        $ref: "#/$defs/app.state.get.SelectionState",
      },
      workspace: {
        description: "Workspace.",
        $ref: "#/$defs/app.state.get.WorkspaceState",
      },
      playback: {
        description: "Playback.",
        $ref: "#/$defs/app.state.get.PlaybackState",
      },
      startScreenVisible: {
        description: "Start screen shown (no project).",
        type: "boolean",
      },
      modal: {
        description: "Modal state (always `null` in the kernel).",
        anyOf: [
          {
            $ref: "#/$defs/app.state.get.Modal",
          },
          {
            type: "null",
          },
        ],
      },
      agentSessions: {
        description: "Connected agent sessions.",
        type: "array",
        items: {
          $ref: "#/$defs/app.state.get.AgentSessionState",
        },
      },
      fullScreen: {
        description: "Full-screen presentation (01 §5.6; explicit `enabled` state, readiness bullet 4).",
        type: "boolean",
      },
    },
    required: [
      "selection",
      "workspace",
      "playback",
      "startScreenVisible",
      "agentSessions",
      "fullScreen",
    ],
  },
  "checkpoint.create.params": {
    title: "NameParams",
    description: "`{name}` parameters.",
    type: "object",
    properties: {
      name: {
        description: "Checkpoint name.",
        type: "string",
        minLength: 1,
        maxLength: 128,
      },
    },
    additionalProperties: false,
    required: [
      "name",
    ],
  },
  "checkpoint.create.result": {
    title: "CheckpointRef",
    description: "`{name, documentRev}`.",
    type: "object",
    properties: {
      name: {
        description: "Name.",
        type: "string",
      },
      documentRev: {
        description: "The document revision it holds.",
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
      },
    },
    required: [
      "name",
      "documentRev",
    ],
  },
  "checkpoint.delete.params": {
    title: "NameParams",
    description: "`{name}` parameters.",
    type: "object",
    properties: {
      name: {
        description: "Checkpoint name.",
        type: "string",
        minLength: 1,
        maxLength: 128,
      },
    },
    additionalProperties: false,
    required: [
      "name",
    ],
  },
  "checkpoint.delete.result": {
    title: "Empty",
    description: "An empty result object.",
    type: "object",
  },
  "checkpoint.list.params": {
    title: "NoParams",
    description: "No parameters.",
    type: "object",
    additionalProperties: false,
  },
  "checkpoint.list.CheckpointEntry": {
    description: "A checkpoint as listed.",
    type: "object",
    properties: {
      name: {
        description: "Name.",
        type: "string",
      },
      createdAt: {
        description: "Creation time.",
        type: "string",
      },
      documentRev: {
        description: "Document revision.",
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
      },
      origin: {
        description: "Who created it.",
        type: "string",
      },
    },
    required: [
      "name",
      "createdAt",
      "documentRev",
      "origin",
    ],
  },
  "checkpoint.list.result": {
    title: "Array_of_CheckpointEntry",
    type: "array",
    items: {
      $ref: "#/$defs/checkpoint.list.CheckpointEntry",
    },
  },
  "checkpoint.restore.params": {
    title: "NameParams",
    description: "`{name}` parameters.",
    type: "object",
    properties: {
      name: {
        description: "Checkpoint name.",
        type: "string",
        minLength: 1,
        maxLength: 128,
      },
    },
    additionalProperties: false,
    required: [
      "name",
    ],
  },
  "checkpoint.restore.result": {
    title: "CheckpointRef",
    description: "`{name, documentRev}`.",
    type: "object",
    properties: {
      name: {
        description: "Name.",
        type: "string",
      },
      documentRev: {
        description: "The document revision it holds.",
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
      },
    },
    required: [
      "name",
      "documentRev",
    ],
  },
  "doc.asset.embed.params": {
    title: "AssetEmbedParams",
    description: "`doc.asset.embed` parameters.",
    type: "object",
    properties: {
      assetIds: {
        description: "Assets.",
        type: [
          "array",
          "null",
        ],
        items: {
          type: "string",
        },
        default: null,
      },
      all: {
        description: "Every linked asset.",
        type: "boolean",
        default: false,
      },
    },
    additionalProperties: false,
  },
  "doc.asset.embed.result": {
    title: "Map_of_AnyValue",
    type: "object",
    additionalProperties: true,
  },
  "doc.asset.export.OnExists": {
    description: "What to do when a target file exists (01 §11.2 rule 4).",
    oneOf: [
      {
        description: "Fail with `fs.exists` / `project.exists`.",
        type: "string",
        const: "fail",
      },
      {
        description: "Overwrite.",
        type: "string",
        const: "overwrite",
      },
    ],
  },
  "doc.asset.export.params": {
    title: "AssetExportParams",
    description: "`doc.asset.export` parameters.",
    type: "object",
    properties: {
      assetId: {
        description: "Asset.",
        type: "string",
      },
      path: {
        description: "Target path.",
        type: "string",
      },
      onExists: {
        description: "Overwrite policy.",
        $ref: "#/$defs/doc.asset.export.OnExists",
      },
    },
    additionalProperties: false,
    required: [
      "assetId",
      "path",
    ],
  },
  "doc.asset.export.result": {
    title: "Map_of_AnyValue",
    type: "object",
    additionalProperties: true,
  },
  "doc.asset.list.AssetKind": {
    description: "Asset kind.",
    oneOf: [
      {
        description: "Sound.",
        type: "string",
        const: "sound",
      },
      {
        description: "Image.",
        type: "string",
        const: "image",
      },
      {
        description: "Movie.",
        type: "string",
        const: "movie",
      },
      {
        description: "Vector file.",
        type: "string",
        const: "vector",
      },
    ],
  },
  "doc.asset.list.AssetStatus": {
    description: "Asset status filter.",
    oneOf: [
      {
        description: "All.",
        type: "string",
        const: "all",
      },
      {
        description: "Missing.",
        type: "string",
        const: "missing",
      },
      {
        description: "Changed.",
        type: "string",
        const: "changed",
      },
      {
        description: "Linked.",
        type: "string",
        const: "linked",
      },
      {
        description: "Embedded.",
        type: "string",
        const: "embedded",
      },
      {
        description: "Unused.",
        type: "string",
        const: "unused",
      },
    ],
  },
  "doc.asset.list.params": {
    title: "AssetListParams",
    description: "`doc.asset.list` parameters.",
    type: "object",
    properties: {
      kind: {
        description: "Kind.",
        anyOf: [
          {
            $ref: "#/$defs/doc.asset.list.AssetKind",
          },
          {
            type: "null",
          },
        ],
      },
      status: {
        description: "Status.",
        $ref: "#/$defs/doc.asset.list.AssetStatus",
      },
    },
    additionalProperties: false,
  },
  "doc.asset.list.result": {
    title: "AssetsResult",
    description: "`{assets}`.",
    type: "object",
    properties: {
      assets: {
        description: "`{id, name, kind, storage, status, path?, size, refs}`.",
        type: "array",
        items: {
          type: "object",
          additionalProperties: true,
        },
      },
    },
    required: [
      "assets",
    ],
  },
  "doc.asset.relink.MatchBy": {
    description: "Relink matching.",
    oneOf: [
      {
        description: "File name.",
        type: "string",
        const: "name",
      },
      {
        description: "Name and size.",
        type: "string",
        const: "nameAndSize",
      },
      {
        description: "Content hash.",
        type: "string",
        const: "hash",
      },
    ],
  },
  "doc.asset.relink.params": {
    title: "AssetRelinkParams",
    description: "`doc.asset.relink` parameters.",
    type: "object",
    properties: {
      assetId: {
        description: "Asset.",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
      path: {
        description: "New path.",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
      searchFolder: {
        description: "Folder to search.",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
      recursive: {
        description: "Search subfolders.",
        type: "boolean",
        default: true,
      },
      matchBy: {
        description: "Matching.",
        $ref: "#/$defs/doc.asset.relink.MatchBy",
      },
    },
    additionalProperties: false,
  },
  "doc.asset.relink.result": {
    title: "Map_of_AnyValue",
    type: "object",
    additionalProperties: true,
  },
  "doc.asset.remove.params": {
    title: "AssetRemoveParams",
    description: "`doc.asset.remove` parameters.",
    type: "object",
    properties: {
      assetIds: {
        description: "Assets.",
        type: [
          "array",
          "null",
        ],
        items: {
          type: "string",
        },
        default: null,
      },
      unused: {
        description: "Every unused asset.",
        type: "boolean",
        default: false,
      },
      force: {
        description: "Remove even if referenced.",
        type: "boolean",
        default: false,
      },
    },
    additionalProperties: false,
  },
  "doc.asset.remove.result": {
    title: "Map_of_AnyValue",
    type: "object",
    additionalProperties: true,
  },
  "doc.asset.status.changed.params": {
    title: "NoParams",
    description: "No parameters.",
    type: "object",
    additionalProperties: false,
  },
  "doc.asset.status.changed.result": {
    title: "Map_of_AnyValue",
    type: "object",
    additionalProperties: true,
  },
  "doc.blob.get.params": {
    title: "BlobParams",
    description: "`doc.blob.get` parameters.",
    type: "object",
    properties: {
      ref: {
        description: "An asset or object holding a blob.",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
      entry: {
        description: "A package entry.",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
      maxBytes: {
        description: "Size limit.",
        type: "integer",
        default: 8388608,
        minimum: 1,
        maximum: 67108864,
      },
    },
    additionalProperties: false,
  },
  "doc.blob.get.result": {
    title: "BlobResult",
    description: "`doc.blob.get` result.",
    type: "object",
    properties: {
      mediaType: {
        description: "Media type.",
        type: "string",
      },
      size: {
        description: "Size.",
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
      },
      sha256: {
        description: "SHA-256.",
        type: "string",
      },
      base64: {
        description: "Content.",
        type: "string",
      },
    },
    required: [
      "mediaType",
      "size",
      "sha256",
      "base64",
    ],
  },
  "doc.changed.params": {
    title: "NoParams",
    description: "No parameters.",
    type: "object",
    additionalProperties: false,
  },
  "doc.changed.ChangeRecord": {
    description: "One object change of a committed (or dry-run) transaction (10 §2.6, 11 §7.2).",
    type: "object",
    properties: {
      id: {
        description: "Object id (compound for container-local ids).",
        type: "string",
      },
      type: {
        description: "Object type.",
        type: "string",
      },
      op: {
        description: "`created`, `modified` or `deleted`.",
        type: "string",
      },
      rev: {
        description: "The object's `rev` after the transaction (the `documentRev` it would get for a dry run).",
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
      },
      parentId: {
        description: "Parent (created and deleted objects).",
        type: [
          "string",
          "null",
        ],
      },
      index: {
        description: "Position in the parent's collection (created objects).",
        type: [
          "integer",
          "null",
        ],
        format: "uint",
        minimum: 0,
      },
      patch: {
        description: "Own-view RFC 6902 patch with `oldValue` (modified objects).",
        type: [
          "array",
          "null",
        ],
        items: true,
      },
    },
    required: [
      "id",
      "type",
      "op",
      "rev",
    ],
  },
  "doc.changed.result": {
    title: "ChangedEvent",
    description: "`doc.changed` payload (10 §2.6).",
    type: "object",
    properties: {
      undoStepId: {
        description: "The step created or replayed.",
        type: "string",
      },
      documentRev: {
        description: "The transaction's revision.",
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
      },
      label: {
        description: "Label.",
        type: "string",
      },
      origin: {
        description: "`user`, `agent:<client>`, `system`, `load`.",
        type: "string",
      },
      changes: {
        description: "Changes (values over 64 KiB elided).",
        type: "array",
        items: {
          $ref: "#/$defs/doc.changed.ChangeRecord",
        },
      },
    },
    required: [
      "undoStepId",
      "documentRev",
      "label",
      "origin",
      "changes",
    ],
  },
  "doc.diff.Detail": {
    description: "Detail of `doc.diff`.",
    oneOf: [
      {
        description: "Without patches.",
        type: "string",
        const: "summary",
      },
      {
        description: "With patches.",
        type: "string",
        const: "full",
      },
    ],
  },
  "doc.diff.params": {
    title: "DiffParams",
    description: "`doc.diff` parameters.",
    type: "object",
    properties: {
      sinceRev: {
        description: "Base revision.",
        type: [
          "integer",
          "null",
        ],
        maximum: 18446744073709551615n,
        minimum: 0,
        default: null,
      },
      sinceCheckpoint: {
        description: "Base checkpoint (by name).",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
      untilRev: {
        description: "End revision (default: current).",
        type: [
          "integer",
          "null",
        ],
        maximum: 18446744073709551615n,
        minimum: 0,
        default: null,
      },
      within: {
        description: "Only objects inside this one.",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
      types: {
        description: "Only these types.",
        type: [
          "array",
          "null",
        ],
        items: {
          type: "string",
        },
        default: null,
      },
      detail: {
        description: "With or without patches.",
        $ref: "#/$defs/doc.diff.Detail",
      },
      limit: {
        description: "Maximum changes.",
        type: "integer",
        format: "uint",
        minimum: 1,
        maximum: 100000,
        default: 1000,
      },
    },
    additionalProperties: false,
  },
  "doc.diff.ChangeRecord": {
    description: "One object change of a committed (or dry-run) transaction (10 §2.6, 11 §7.2).",
    type: "object",
    properties: {
      id: {
        description: "Object id (compound for container-local ids).",
        type: "string",
      },
      type: {
        description: "Object type.",
        type: "string",
      },
      op: {
        description: "`created`, `modified` or `deleted`.",
        type: "string",
      },
      rev: {
        description: "The object's `rev` after the transaction (the `documentRev` it would get for a dry run).",
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
      },
      parentId: {
        description: "Parent (created and deleted objects).",
        type: [
          "string",
          "null",
        ],
      },
      index: {
        description: "Position in the parent's collection (created objects).",
        type: [
          "integer",
          "null",
        ],
        format: "uint",
        minimum: 0,
      },
      patch: {
        description: "Own-view RFC 6902 patch with `oldValue` (modified objects).",
        type: [
          "array",
          "null",
        ],
        items: true,
      },
    },
    required: [
      "id",
      "type",
      "op",
      "rev",
    ],
  },
  "doc.diff.result": {
    title: "DiffResult",
    description: "`doc.diff` result.",
    type: "object",
    properties: {
      fromRev: {
        description: "Base revision.",
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
      },
      toRev: {
        description: "End revision.",
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
      },
      changes: {
        description: "Net changes.",
        type: "array",
        items: {
          $ref: "#/$defs/doc.diff.ChangeRecord",
        },
      },
      truncated: {
        description: "More changes than `limit`.",
        type: "boolean",
      },
    },
    required: [
      "fromRev",
      "toRev",
      "changes",
      "truncated",
    ],
  },
  "doc.export.json.ExportScope": {
    description: "Export scope.",
    oneOf: [
      {
        description: "The whole project.",
        type: "string",
        const: "project",
      },
      {
        description: "Scenes.",
        type: "string",
        const: "scenes",
      },
      {
        description: "Elements.",
        type: "string",
        const: "elements",
      },
    ],
  },
  "doc.export.json.Binaries": {
    description: "Binary handling.",
    oneOf: [
      {
        description: "Inline as base64.",
        type: "string",
        const: "embed",
      },
      {
        description: "As references.",
        type: "string",
        const: "reference",
      },
      {
        description: "Left out.",
        type: "string",
        const: "omit",
      },
    ],
  },
  "doc.export.json.OnExists": {
    description: "What to do when a target file exists (01 §11.2 rule 4).",
    oneOf: [
      {
        description: "Fail with `fs.exists` / `project.exists`.",
        type: "string",
        const: "fail",
      },
      {
        description: "Overwrite.",
        type: "string",
        const: "overwrite",
      },
    ],
  },
  "doc.export.json.params": {
    title: "ExportJsonParams",
    description: "`doc.export.json` parameters.",
    type: "object",
    properties: {
      scope: {
        description: "What to export.",
        $ref: "#/$defs/doc.export.json.ExportScope",
      },
      refs: {
        description: "Scenes or elements.",
        type: [
          "array",
          "null",
        ],
        items: {
          type: "string",
        },
        default: null,
      },
      path: {
        description: "Target file (default: inline).",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
      binaries: {
        description: "Binary data.",
        $ref: "#/$defs/doc.export.json.Binaries",
      },
      pretty: {
        description: "Indented.",
        type: "boolean",
        default: true,
      },
      onExists: {
        description: "Overwrite policy.",
        $ref: "#/$defs/doc.export.json.OnExists",
      },
    },
    additionalProperties: false,
    required: [
      "scope",
    ],
  },
  "doc.export.json.result": {
    title: "ExportJsonResult",
    description: "`doc.export.json` result.",
    type: "object",
    properties: {
      bundle: {
        description: "Inline bundle.",
        type: [
          "object",
          "null",
        ],
        additionalProperties: true,
      },
      path: {
        description: "File written.",
        type: [
          "string",
          "null",
        ],
      },
      bytes: {
        description: "Size.",
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
      },
      objectCount: {
        description: "Objects.",
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
      },
    },
    required: [
      "bytes",
      "objectCount",
    ],
  },
  "doc.import.json.IdPolicy": {
    description: "ID policy.",
    oneOf: [
      {
        description: "New IDs.",
        type: "string",
        const: "new",
      },
      {
        description: "Keep IDs.",
        type: "string",
        const: "preserve",
      },
    ],
  },
  "doc.import.json.OnConflict": {
    description: "Conflict policy.",
    oneOf: [
      {
        description: "Rename.",
        type: "string",
        const: "rename",
      },
      {
        description: "Replace.",
        type: "string",
        const: "replace",
      },
      {
        description: "Fail.",
        type: "string",
        const: "fail",
      },
    ],
  },
  "doc.import.json.ColourPolicy": {
    description: "Colour policy.",
    oneOf: [
      {
        description: "Keep existing colours.",
        type: "string",
        const: "keepExisting",
      },
      {
        description: "Update existing colours.",
        type: "string",
        const: "updateExisting",
      },
    ],
  },
  "doc.import.json.params": {
    title: "ImportJsonParams",
    description: "`doc.import.json` parameters.",
    type: "object",
    properties: {
      source: {
        description: "A path or an inline bundle.",
      },
      target: {
        description: "Target scene (omit: new scenes).",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
      index: {
        description: "Scene position.",
        type: [
          "integer",
          "null",
        ],
        maximum: 2147483647,
        minimum: 0,
        default: null,
      },
      atFrame: {
        description: "First frame.",
        type: "integer",
        default: 1,
        minimum: 1,
        maximum: 100000,
      },
      idPolicy: {
        description: "ID policy.",
        $ref: "#/$defs/doc.import.json.IdPolicy",
      },
      onConflict: {
        description: "Conflict policy.",
        $ref: "#/$defs/doc.import.json.OnConflict",
      },
      colours: {
        description: "Colour policy.",
        $ref: "#/$defs/doc.import.json.ColourPolicy",
      },
      dryRun: {
        description: "Report only.",
        type: "boolean",
        default: false,
      },
    },
    additionalProperties: false,
    required: [
      "source",
    ],
  },
  "doc.import.json.result": {
    title: "Map_of_AnyValue",
    type: "object",
    additionalProperties: true,
  },
  "doc.loadReport.get.params": {
    title: "NoParams",
    description: "No parameters.",
    type: "object",
    additionalProperties: false,
  },
  "doc.loadReport.get.result": {
    title: "Map_of_AnyValue",
    type: "object",
    additionalProperties: true,
  },
  "doc.object.find.Sort": {
    description: "Sort order of `doc.object.find`.",
    oneOf: [
      {
        description: "Canonical path (AG-06).",
        type: "string",
        const: "path",
      },
      {
        description: "Name.",
        type: "string",
        const: "name",
      },
      {
        description: "Revision.",
        type: "string",
        const: "rev",
      },
      {
        description: "Creation time.",
        type: "string",
        const: "createdAt",
      },
    ],
  },
  "doc.object.find.params": {
    title: "FindParams",
    description: "`doc.object.find` parameters.",
    type: "object",
    properties: {
      type: {
        description: "Object type (or alias).",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
      within: {
        description: "Only descendants of this object.",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
      name: {
        description: "Name: exact, or a glob with `*` and `?`.",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
      where: {
        description: "Predicates (10 §10.3).",
        type: [
          "object",
          "null",
        ],
        additionalProperties: true,
        default: null,
      },
      limit: {
        description: "Page size.",
        type: "integer",
        format: "uint",
        minimum: 1,
        maximum: 1000,
        default: 100,
      },
      offset: {
        description: "First item.",
        type: "integer",
        format: "uint",
        minimum: 0,
        maximum: 100000000,
        default: 0,
      },
      fields: {
        description: "Extra top-level properties per item.",
        type: [
          "array",
          "null",
        ],
        items: {
          type: "string",
        },
        default: null,
      },
      sort: {
        description: "Order.",
        $ref: "#/$defs/doc.object.find.Sort",
      },
    },
    additionalProperties: false,
  },
  "doc.object.find.result": {
    title: "FindResult",
    description: "`doc.object.find` result.",
    type: "object",
    properties: {
      items: {
        description: "Items `{id, type, name, path, rev, …fields}`.",
        type: "array",
        items: {
          type: "object",
          additionalProperties: true,
        },
      },
      total: {
        description: "Matches in total.",
        type: "integer",
        format: "uint",
        minimum: 0,
      },
      nextOffset: {
        description: "Offset of the next page (`null` at the end).",
        type: [
          "integer",
          "null",
        ],
        format: "uint",
        minimum: 0,
      },
    },
    required: [
      "items",
      "total",
    ],
  },
  "doc.object.get.Include": {
    description: "What to add to returned objects.",
    oneOf: [
      {
        description: "Derived properties (10 §2.7, AG-04).",
        type: "string",
        const: "derived",
      },
      {
        description: "Size statistics, returned beside the object (ADR-0013 §9; SI-015).",
        type: "string",
        const: "stats",
      },
    ],
  },
  "doc.object.get.Page": {
    description: "Paging of one large array inside the object (AG-05).",
    type: "object",
    properties: {
      pointer: {
        description: "JSON Pointer of the array inside the object.",
        type: "string",
      },
      offset: {
        description: "First item.",
        type: "integer",
        format: "uint",
        minimum: 0,
        maximum: 100000000,
        default: 0,
      },
      limit: {
        description: "Items.",
        type: "integer",
        format: "uint",
        minimum: 1,
        maximum: 100000,
        default: 1000,
      },
    },
    additionalProperties: false,
    required: [
      "pointer",
    ],
  },
  "doc.object.get.params": {
    title: "GetParams",
    description: "`doc.object.get` parameters.",
    type: "object",
    properties: {
      ref: {
        description: "One object (id, compound id, path or special reference).",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
      refs: {
        description: "Several objects (`@selection` expands to the selection).",
        type: [
          "array",
          "null",
        ],
        items: {
          type: "string",
        },
        minItems: 1,
        maxItems: 1000,
        default: null,
      },
      depth: {
        description: "Object levels in full: 1 = children as stubs, 0 = the object as a stub, -1 = everything (SI-011).",
        type: "integer",
        default: 1,
        minimum: -1,
        maximum: 20,
      },
      fields: {
        description: "Top-level properties to keep (`id` and `type` are always kept).",
        type: [
          "array",
          "null",
        ],
        items: {
          type: "string",
        },
        default: null,
      },
      include: {
        description: "Extra data.",
        type: "array",
        items: {
          $ref: "#/$defs/doc.object.get.Include",
        },
      },
      page: {
        description: "Page through one large array.",
        anyOf: [
          {
            $ref: "#/$defs/doc.object.get.Page",
          },
          {
            type: "null",
          },
        ],
      },
    },
    additionalProperties: false,
  },
  "doc.object.get.PageInfo": {
    description: "Paging information.",
    type: "object",
    properties: {
      pointer: {
        description: "The paged array.",
        type: "string",
      },
      total: {
        description: "Items in the array.",
        type: "integer",
        format: "uint",
        minimum: 0,
      },
      nextOffset: {
        description: "Offset of the next page (`null` at the end).",
        type: [
          "integer",
          "null",
        ],
        format: "uint",
        minimum: 0,
      },
    },
    required: [
      "pointer",
      "total",
    ],
  },
  "doc.object.get.result": {
    title: "GetResult",
    description: "`doc.object.get` result.",
    type: "object",
    properties: {
      object: {
        description: "The object (single `ref`).",
        type: [
          "object",
          "null",
        ],
        additionalProperties: true,
      },
      objects: {
        description: "The objects (`refs`).",
        type: [
          "array",
          "null",
        ],
        items: {
          type: "object",
          additionalProperties: true,
        },
      },
      stats: {
        description: "Statistics per object (`include: [\"stats\"]`).",
        type: [
          "array",
          "null",
        ],
        items: true,
      },
      page: {
        description: "Paging of the array named by `page.pointer`, per object.",
        type: [
          "array",
          "null",
        ],
        items: {
          $ref: "#/$defs/doc.object.get.PageInfo",
        },
      },
    },
  },
  "doc.object.patch.OpParam": {
    description: "One RFC 6902 operation, pointers relative to the object.",
    type: "object",
    properties: {
      op: {
        description: "Operation.",
        $ref: "#/$defs/doc.object.patch.OpKind",
      },
      path: {
        description: "Target pointer.",
        type: "string",
        maxLength: 65536,
      },
      value: {
        description: "Value (`add`, `replace`, `test`).",
        default: null,
      },
      from: {
        description: "Source pointer (`move`, `copy`).",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
    },
    additionalProperties: false,
    required: [
      "op",
      "path",
    ],
  },
  "doc.object.patch.OpKind": {
    description: "RFC 6902 operation names.",
    oneOf: [
      {
        description: "add.",
        type: "string",
        const: "add",
      },
      {
        description: "remove.",
        type: "string",
        const: "remove",
      },
      {
        description: "replace.",
        type: "string",
        const: "replace",
      },
      {
        description: "move.",
        type: "string",
        const: "move",
      },
      {
        description: "copy.",
        type: "string",
        const: "copy",
      },
      {
        description: "test.",
        type: "string",
        const: "test",
      },
    ],
  },
  "doc.object.patch.ExpectRev": {
    description: "`expectRev`: one revision for every target, or per reference.",
    anyOf: [
      {
        description: "Applies to every target.",
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
      },
      {
        description: "Reference → revision.",
        type: "object",
        additionalProperties: {
          type: "integer",
          maximum: 18446744073709551615n,
          minimum: 0,
        },
      },
    ],
  },
  "doc.object.patch.params": {
    title: "PatchParams",
    description: "`doc.object.patch` parameters.",
    type: "object",
    properties: {
      targets: {
        description: "Objects to patch (ids, paths, `@selection`).",
        type: "array",
        items: {
          type: "string",
        },
        minItems: 1,
        maxItems: 1000,
      },
      props: {
        description: "JSON merge patch (RFC 7386) applied to every target.",
        type: [
          "object",
          "null",
        ],
        additionalProperties: true,
        default: null,
      },
      ops: {
        description: "JSON Patch operations (RFC 6902), pointers relative to each target.",
        type: [
          "array",
          "null",
        ],
        items: {
          $ref: "#/$defs/doc.object.patch.OpParam",
        },
        maxItems: 10000,
      },
      expectRev: {
        description: "Optimistic concurrency (REV-05).",
        anyOf: [
          {
            $ref: "#/$defs/doc.object.patch.ExpectRev",
          },
          {
            type: "null",
          },
        ],
      },
      label: {
        description: "Undo label (default \"Set <property>\").",
        type: [
          "string",
          "null",
        ],
        minLength: 1,
        maxLength: 200,
        default: null,
      },
      dryRun: {
        description: "Report the changes without applying them (the executor reads it before the handler runs).",
        type: "boolean",
        default: false,
      },
    },
    additionalProperties: false,
    required: [
      "targets",
    ],
  },
  "doc.object.patch.ChangeRecord": {
    description: "One object change of a committed (or dry-run) transaction (10 §2.6, 11 §7.2).",
    type: "object",
    properties: {
      id: {
        description: "Object id (compound for container-local ids).",
        type: "string",
      },
      type: {
        description: "Object type.",
        type: "string",
      },
      op: {
        description: "`created`, `modified` or `deleted`.",
        type: "string",
      },
      rev: {
        description: "The object's `rev` after the transaction (the `documentRev` it would get for a dry run).",
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
      },
      parentId: {
        description: "Parent (created and deleted objects).",
        type: [
          "string",
          "null",
        ],
      },
      index: {
        description: "Position in the parent's collection (created objects).",
        type: [
          "integer",
          "null",
        ],
        format: "uint",
        minimum: 0,
      },
      patch: {
        description: "Own-view RFC 6902 patch with `oldValue` (modified objects).",
        type: [
          "array",
          "null",
        ],
        items: true,
      },
    },
    required: [
      "id",
      "type",
      "op",
      "rev",
    ],
  },
  "doc.object.patch.result": {
    title: "PatchResult",
    description: "`doc.object.patch` result.",
    type: "object",
    properties: {
      changes: {
        description: "The changes (also in the envelope).",
        type: "array",
        items: {
          $ref: "#/$defs/doc.object.patch.ChangeRecord",
        },
      },
      documentRev: {
        description: "`documentRev` after the patch (the revision it would get for a dry run).",
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
      },
    },
    required: [
      "changes",
      "documentRev",
    ],
  },
  "doc.object.refs.RefDirection": {
    description: "Reference direction.",
    oneOf: [
      {
        description: "Objects referencing it.",
        type: "string",
        const: "incoming",
      },
      {
        description: "Objects it references.",
        type: "string",
        const: "outgoing",
      },
    ],
  },
  "doc.object.refs.params": {
    title: "RefsParams",
    description: "`doc.object.refs` parameters.",
    type: "object",
    properties: {
      ref: {
        description: "The object.",
        type: "string",
      },
      direction: {
        description: "Incoming or outgoing.",
        $ref: "#/$defs/doc.object.refs.RefDirection",
      },
      limit: {
        description: "Maximum.",
        type: "integer",
        format: "uint",
        minimum: 1,
        maximum: 100000,
        default: 500,
      },
    },
    additionalProperties: false,
    required: [
      "ref",
    ],
  },
  "doc.object.refs.result": {
    title: "RefsResult",
    description: "`doc.object.refs` result.",
    type: "object",
    properties: {
      refs: {
        description: "`{id, type, path, property}`.",
        type: "array",
        items: {
          type: "object",
          additionalProperties: true,
        },
      },
      total: {
        description: "Total.",
        type: "integer",
        format: "uint",
        minimum: 0,
      },
    },
    required: [
      "refs",
      "total",
    ],
  },
  "doc.object.resolve.params": {
    title: "ResolveParams",
    description: "`doc.object.resolve` parameters.",
    type: "object",
    properties: {
      refs: {
        description: "References (ids, compound ids, paths, special references).",
        type: "array",
        items: {
          type: "string",
        },
        minItems: 1,
        maxItems: 1000,
      },
    },
    additionalProperties: false,
    required: [
      "refs",
    ],
  },
  "doc.object.resolve.result": {
    title: "ResolveResult",
    description: "`doc.object.resolve` result.",
    type: "object",
    properties: {
      items: {
        description: "One item per reference: `{ref, id, type, name, path, rev}` or `{ref, error}`.",
        type: "array",
        items: {
          type: "object",
          additionalProperties: true,
        },
      },
    },
    required: [
      "items",
    ],
  },
  "doc.package.changedOnDisk.params": {
    title: "NoParams",
    description: "No parameters.",
    type: "object",
    additionalProperties: false,
  },
  "doc.package.changedOnDisk.result": {
    title: "Map_of_AnyValue",
    type: "object",
    additionalProperties: true,
  },
  "doc.package.export.Form": {
    description: "Package form.",
    oneOf: [
      {
        description: "Zip package.",
        type: "string",
        const: "package",
      },
      {
        description: "Folder.",
        type: "string",
        const: "folder",
      },
    ],
  },
  "doc.package.export.OnExists": {
    description: "What to do when a target file exists (01 §11.2 rule 4).",
    oneOf: [
      {
        description: "Fail with `fs.exists` / `project.exists`.",
        type: "string",
        const: "fail",
      },
      {
        description: "Overwrite.",
        type: "string",
        const: "overwrite",
      },
    ],
  },
  "doc.package.export.params": {
    title: "PackageExportParams",
    description: "`doc.package.export` parameters.",
    type: "object",
    properties: {
      path: {
        description: "Target path.",
        type: "string",
      },
      form: {
        description: "Form.",
        $ref: "#/$defs/doc.package.export.Form",
      },
      onExists: {
        description: "Overwrite policy.",
        $ref: "#/$defs/doc.package.export.OnExists",
      },
    },
    additionalProperties: false,
    required: [
      "path",
    ],
  },
  "doc.package.export.result": {
    title: "JobStarted",
    description: "`{jobId}`.",
    type: "object",
    properties: {
      jobId: {
        description: "The job.",
        type: "string",
      },
    },
    required: [
      "jobId",
    ],
  },
  "doc.package.get.params": {
    title: "PackageGetParams",
    description: "`doc.package.get` parameters.",
    type: "object",
    properties: {
      path: {
        description: "Package path.",
        type: "string",
      },
      validate: {
        description: "Validate fully.",
        type: "boolean",
        default: false,
      },
    },
    additionalProperties: false,
    required: [
      "path",
    ],
  },
  "doc.package.get.result": {
    title: "Map_of_AnyValue",
    type: "object",
    additionalProperties: true,
  },
  "doc.repair.params": {
    title: "RepairParams",
    description: "`doc.repair` parameters.",
    type: "object",
    properties: {
      issueIds: {
        description: "Issues to fix.",
        type: [
          "array",
          "null",
        ],
        items: {
          type: "string",
        },
        default: null,
      },
      codes: {
        description: "Issue codes to fix.",
        type: [
          "array",
          "null",
        ],
        items: {
          type: "string",
        },
        default: null,
      },
      within: {
        description: "Subtree.",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
      all: {
        description: "Every fixable issue.",
        type: "boolean",
        default: false,
      },
      dryRun: {
        description: "Report only.",
        type: "boolean",
        default: false,
      },
    },
    additionalProperties: false,
  },
  "doc.repair.ChangeRecord": {
    description: "One object change of a committed (or dry-run) transaction (10 §2.6, 11 §7.2).",
    type: "object",
    properties: {
      id: {
        description: "Object id (compound for container-local ids).",
        type: "string",
      },
      type: {
        description: "Object type.",
        type: "string",
      },
      op: {
        description: "`created`, `modified` or `deleted`.",
        type: "string",
      },
      rev: {
        description: "The object's `rev` after the transaction (the `documentRev` it would get for a dry run).",
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
      },
      parentId: {
        description: "Parent (created and deleted objects).",
        type: [
          "string",
          "null",
        ],
      },
      index: {
        description: "Position in the parent's collection (created objects).",
        type: [
          "integer",
          "null",
        ],
        format: "uint",
        minimum: 0,
      },
      patch: {
        description: "Own-view RFC 6902 patch with `oldValue` (modified objects).",
        type: [
          "array",
          "null",
        ],
        items: true,
      },
    },
    required: [
      "id",
      "type",
      "op",
      "rev",
    ],
  },
  "doc.repair.result": {
    title: "RepairResult",
    description: "`doc.repair` result.",
    type: "object",
    properties: {
      fixed: {
        description: "Fixed issues.",
        type: "array",
        items: {
          type: "string",
        },
      },
      skipped: {
        description: "`{issueId, reason}`.",
        type: "array",
        items: {
          type: "object",
          additionalProperties: true,
        },
      },
      changes: {
        description: "Changes.",
        type: "array",
        items: {
          $ref: "#/$defs/doc.repair.ChangeRecord",
        },
      },
    },
    required: [
      "fixed",
      "skipped",
      "changes",
    ],
  },
  "doc.schema.get.params": {
    title: "SchemaGetParams",
    description: "`doc.schema.get` parameters.",
    type: "object",
    properties: {
      type: {
        description: "Object type (or alias).",
        type: "string",
      },
      property: {
        description: "JSON Pointer of one property.",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
    },
    additionalProperties: false,
    required: [
      "type",
    ],
  },
  "doc.schema.get.result": {
    title: "SchemaGetResult",
    description: "`doc.schema.get` result.",
    type: "object",
    properties: {
      type: {
        description: "Type name.",
        type: "string",
      },
      formatVersion: {
        description: "Format version.",
        type: "string",
      },
      schema: {
        description: "The JSON Schema (draft 2020-12).",
        type: "object",
        additionalProperties: true,
      },
    },
    required: [
      "type",
      "formatVersion",
      "schema",
    ],
  },
  "doc.schema.list.params": {
    title: "NoParams",
    description: "No parameters.",
    type: "object",
    additionalProperties: false,
  },
  "doc.schema.list.TypeInfo": {
    description: "One type in `doc.schema.list`.",
    type: "object",
    properties: {
      type: {
        description: "Type name.",
        type: "string",
      },
      aliases: {
        description: "Path aliases.",
        type: "array",
        items: {
          type: "string",
        },
      },
      parents: {
        description: "Possible parent types.",
        type: "array",
        items: {
          type: "string",
        },
      },
      nameAddressable: {
        description: "Addressable by quoted name in paths.",
        type: "boolean",
      },
      description: {
        description: "Description.",
        type: "string",
      },
    },
    required: [
      "type",
      "aliases",
      "parents",
      "nameAddressable",
      "description",
    ],
  },
  "doc.schema.list.result": {
    title: "SchemaListResult",
    description: "`doc.schema.list` result.",
    type: "object",
    properties: {
      formatVersion: {
        description: "Format version.",
        type: "string",
      },
      types: {
        description: "Types.",
        type: "array",
        items: {
          $ref: "#/$defs/doc.schema.list.TypeInfo",
        },
      },
    },
    required: [
      "formatVersion",
      "types",
    ],
  },
  "doc.schema.validate.ValidateMode": {
    description: "Validation mode.",
    oneOf: [
      {
        description: "A new object.",
        type: "string",
        const: "create",
      },
      {
        description: "A replacement.",
        type: "string",
        const: "replace",
      },
    ],
  },
  "doc.schema.validate.params": {
    title: "SchemaValidateParams",
    description: "`doc.schema.validate` parameters.",
    type: "object",
    properties: {
      type: {
        description: "Object type.",
        type: "string",
      },
      object: {
        description: "Candidate object.",
        type: "object",
        additionalProperties: true,
      },
      mode: {
        description: "Create (id/rev may be absent) or replace.",
        $ref: "#/$defs/doc.schema.validate.ValidateMode",
      },
    },
    additionalProperties: false,
    required: [
      "type",
      "object",
    ],
  },
  "doc.schema.validate.result": {
    title: "ValidateResult",
    description: "Validation result.",
    type: "object",
    properties: {
      valid: {
        description: "No errors.",
        type: "boolean",
      },
      errors: {
        description: "`{pointer, code, message}`.",
        type: "array",
        items: {
          type: "object",
          additionalProperties: true,
        },
      },
    },
    required: [
      "valid",
      "errors",
    ],
  },
  "doc.state.get.params": {
    title: "NoParams",
    description: "No parameters.",
    type: "object",
    additionalProperties: false,
  },
  "doc.state.get.result": {
    title: "StateResult",
    description: "`doc.state.get` result.",
    type: "object",
    properties: {
      projectId: {
        description: "Project id.",
        type: "string",
      },
      path: {
        description: "Package path (`null` until saved).",
        type: [
          "string",
          "null",
        ],
      },
      form: {
        description: "`package` or `folder` (`null` until saved).",
        type: [
          "string",
          "null",
        ],
      },
      formatVersion: {
        description: "Format version.",
        type: "string",
      },
      migratedFrom: {
        description: "Version migrated from on load (C1.3).",
        type: [
          "string",
          "null",
        ],
      },
      documentRev: {
        description: "Current revision.",
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
      },
      savedRev: {
        description: "Revision at the last save.",
        type: [
          "integer",
          "null",
        ],
        maximum: 18446744073709551615n,
        minimum: 0,
      },
      dirty: {
        description: "Modified since the last save.",
        type: "boolean",
      },
      readOnly: {
        description: "Read-only.",
        type: "boolean",
      },
      lock: {
        description: "Lock holder (C1.3).",
      },
      historyFromRev: {
        description: "Oldest revision `doc.diff` accepts.",
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
      },
    },
    required: [
      "projectId",
      "formatVersion",
      "documentRev",
      "dirty",
      "readOnly",
      "historyFromRev",
    ],
  },
  "doc.validate.Severity": {
    description: "Severity filter.",
    oneOf: [
      {
        description: "Everything.",
        type: "string",
        const: "info",
      },
      {
        description: "Warnings and errors.",
        type: "string",
        const: "warning",
      },
      {
        description: "Errors.",
        type: "string",
        const: "error",
      },
    ],
  },
  "doc.validate.params": {
    title: "ValidateParams",
    description: "`doc.validate` parameters.",
    type: "object",
    properties: {
      within: {
        description: "Subtree.",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
      checks: {
        description: "Checks (default all).",
        type: [
          "array",
          "null",
        ],
        items: {
          type: "string",
        },
        default: null,
      },
      minSeverity: {
        description: "Minimum severity.",
        $ref: "#/$defs/doc.validate.Severity",
      },
      limit: {
        description: "Maximum issues.",
        type: "integer",
        format: "uint",
        minimum: 1,
        maximum: 100000,
        default: 500,
      },
    },
    additionalProperties: false,
  },
  "doc.validate.result": {
    title: "LintResult",
    description: "`doc.validate` result.",
    type: "object",
    properties: {
      documentRev: {
        description: "Revision checked.",
        type: "integer",
        maximum: 18446744073709551615n,
        minimum: 0,
      },
      issues: {
        description: "Issues (10 §10.4 `Issue`).",
        type: "array",
        items: {
          type: "object",
          additionalProperties: true,
        },
      },
      counts: {
        description: "Counts by severity.",
        type: "object",
        additionalProperties: {
          type: "integer",
          format: "uint",
          minimum: 0,
        },
      },
      truncated: {
        description: "More issues than `limit`.",
        type: "boolean",
      },
    },
    required: [
      "documentRev",
      "issues",
      "counts",
      "truncated",
    ],
  },
  "edit.clipboard.changed.params": {
    title: "NoParams",
    description: "No parameters.",
    type: "object",
    additionalProperties: false,
  },
  "edit.clipboard.changed.ClipboardKind": {
    description: "Clipboard content kinds (01 §12.4).",
    oneOf: [
      {
        description: "Drawing objects.",
        type: "string",
        const: "drawingObjects",
      },
      {
        description: "Elements.",
        type: "string",
        const: "elements",
      },
      {
        description: "Element properties.",
        type: "string",
        const: "elementProperties",
      },
      {
        description: "Cells.",
        type: "string",
        const: "cells",
      },
      {
        description: "Keyframes.",
        type: "string",
        const: "keyframes",
      },
      {
        description: "A motion point.",
        type: "string",
        const: "motionPoint",
      },
      {
        description: "Library templates.",
        type: "string",
        const: "templates",
      },
      {
        description: "Function points.",
        type: "string",
        const: "functionPoints",
      },
      {
        description: "An image from the system clipboard.",
        type: "string",
        const: "image",
      },
    ],
  },
  "edit.clipboard.changed.result": {
    title: "ClipboardSummary",
    description: "`{kind, itemCount}`.",
    type: "object",
    properties: {
      kind: {
        description: "Kind.",
        $ref: "#/$defs/edit.clipboard.changed.ClipboardKind",
      },
      itemCount: {
        description: "Items.",
        type: "integer",
        maximum: 2147483647,
        minimum: 0,
      },
    },
    required: [
      "kind",
      "itemCount",
    ],
  },
  "edit.clipboard.get.params": {
    title: "ClipboardGetParams",
    description: "`edit.clipboard.get` parameters.",
    type: "object",
    properties: {
      includeData: {
        description: "Include the items.",
        type: "boolean",
        default: false,
      },
    },
    additionalProperties: false,
  },
  "edit.clipboard.get.Clipboard": {
    description: "`Clipboard` (01 §12.4).",
    type: "object",
    properties: {
      kind: {
        description: "Content kind.",
        $ref: "#/$defs/edit.clipboard.get.ClipboardKind",
      },
      sourceContext: {
        description: "Context it was copied from.",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
      sourceProjectId: {
        description: "Project it was copied from.",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
      itemCount: {
        description: "Number of items.",
        type: "integer",
        maximum: 1000000,
        minimum: 0,
      },
      items: {
        description: "Items in the section-10 shape of the kind (present only with `includeData`).",
        type: [
          "array",
          "null",
        ],
        items: {
          type: "object",
          additionalProperties: true,
        },
        maxItems: 1000000,
      },
      options: {
        description: "Cut-special options.",
        type: [
          "object",
          "null",
        ],
        additionalProperties: true,
        default: null,
      },
    },
    additionalProperties: false,
    required: [
      "kind",
      "itemCount",
    ],
  },
  "edit.clipboard.get.ClipboardKind": {
    description: "Clipboard content kinds (01 §12.4).",
    oneOf: [
      {
        description: "Drawing objects.",
        type: "string",
        const: "drawingObjects",
      },
      {
        description: "Elements.",
        type: "string",
        const: "elements",
      },
      {
        description: "Element properties.",
        type: "string",
        const: "elementProperties",
      },
      {
        description: "Cells.",
        type: "string",
        const: "cells",
      },
      {
        description: "Keyframes.",
        type: "string",
        const: "keyframes",
      },
      {
        description: "A motion point.",
        type: "string",
        const: "motionPoint",
      },
      {
        description: "Library templates.",
        type: "string",
        const: "templates",
      },
      {
        description: "Function points.",
        type: "string",
        const: "functionPoints",
      },
      {
        description: "An image from the system clipboard.",
        type: "string",
        const: "image",
      },
    ],
  },
  "edit.clipboard.get.result": {
    title: "ClipboardGetResult",
    description: "`edit.clipboard.get` result: the clipboard, or `null` when it is empty.",
    type: "object",
    properties: {
      clipboard: {
        description: "Content.",
        anyOf: [
          {
            $ref: "#/$defs/edit.clipboard.get.Clipboard",
          },
          {
            type: "null",
          },
        ],
      },
    },
  },
  "edit.clipboard.set.Clipboard": {
    description: "`Clipboard` (01 §12.4).",
    type: "object",
    properties: {
      kind: {
        description: "Content kind.",
        $ref: "#/$defs/edit.clipboard.set.ClipboardKind",
      },
      sourceContext: {
        description: "Context it was copied from.",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
      sourceProjectId: {
        description: "Project it was copied from.",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
      itemCount: {
        description: "Number of items.",
        type: "integer",
        maximum: 1000000,
        minimum: 0,
      },
      items: {
        description: "Items in the section-10 shape of the kind (present only with `includeData`).",
        type: [
          "array",
          "null",
        ],
        items: {
          type: "object",
          additionalProperties: true,
        },
        maxItems: 1000000,
      },
      options: {
        description: "Cut-special options.",
        type: [
          "object",
          "null",
        ],
        additionalProperties: true,
        default: null,
      },
    },
    additionalProperties: false,
    required: [
      "kind",
      "itemCount",
    ],
  },
  "edit.clipboard.set.ClipboardKind": {
    description: "Clipboard content kinds (01 §12.4).",
    oneOf: [
      {
        description: "Drawing objects.",
        type: "string",
        const: "drawingObjects",
      },
      {
        description: "Elements.",
        type: "string",
        const: "elements",
      },
      {
        description: "Element properties.",
        type: "string",
        const: "elementProperties",
      },
      {
        description: "Cells.",
        type: "string",
        const: "cells",
      },
      {
        description: "Keyframes.",
        type: "string",
        const: "keyframes",
      },
      {
        description: "A motion point.",
        type: "string",
        const: "motionPoint",
      },
      {
        description: "Library templates.",
        type: "string",
        const: "templates",
      },
      {
        description: "Function points.",
        type: "string",
        const: "functionPoints",
      },
      {
        description: "An image from the system clipboard.",
        type: "string",
        const: "image",
      },
    ],
  },
  "edit.clipboard.set.params": {
    title: "ClipboardSetParams",
    description: "`edit.clipboard.set` parameters.",
    type: "object",
    properties: {
      content: {
        description: "The content.",
        $ref: "#/$defs/edit.clipboard.set.Clipboard",
      },
    },
    additionalProperties: false,
    required: [
      "content",
    ],
  },
  "edit.clipboard.set.result": {
    title: "ClipboardSummary",
    description: "`{kind, itemCount}`.",
    type: "object",
    properties: {
      kind: {
        description: "Kind.",
        $ref: "#/$defs/edit.clipboard.set.ClipboardKind",
      },
      itemCount: {
        description: "Items.",
        type: "integer",
        maximum: 2147483647,
        minimum: 0,
      },
    },
    required: [
      "kind",
      "itemCount",
    ],
  },
  "edit.copy.EditContext": {
    description: "Selection / clipboard context (01 §6.2); `auto` follows the focused panel.",
    oneOf: [
      {
        description: "The focused panel's context.",
        type: "string",
        const: "auto",
      },
      {
        description: "Drawing objects of the current drawing.",
        type: "string",
        const: "drawing",
      },
      {
        description: "Elements in the camera/top/side views.",
        type: "string",
        const: "staging",
      },
      {
        description: "Exposure Sheet.",
        type: "string",
        const: "xsheet",
      },
      {
        description: "Timeline.",
        type: "string",
        const: "timeline",
      },
      {
        description: "Library.",
        type: "string",
        const: "library",
      },
      {
        description: "Function editor.",
        type: "string",
        const: "functionEditor",
      },
    ],
  },
  "edit.copy.params": {
    title: "ObjectsParams",
    description: "`context` + `objectIds` parameters of cut, copy and delete.",
    type: "object",
    properties: {
      context: {
        description: "Context; `auto` follows the focused panel.",
        $ref: "#/$defs/edit.copy.EditContext",
        default: "auto",
      },
      objectIds: {
        description: "Objects (default: the selection).",
        type: [
          "array",
          "null",
        ],
        items: {
          type: "string",
        },
        maxItems: 100000,
        default: null,
      },
    },
    additionalProperties: false,
  },
  "edit.copy.ClipboardSummary": {
    description: "`{kind, itemCount}`.",
    type: "object",
    properties: {
      kind: {
        description: "Kind.",
        $ref: "#/$defs/edit.copy.ClipboardKind",
      },
      itemCount: {
        description: "Items.",
        type: "integer",
        maximum: 2147483647,
        minimum: 0,
      },
    },
    required: [
      "kind",
      "itemCount",
    ],
  },
  "edit.copy.ClipboardKind": {
    description: "Clipboard content kinds (01 §12.4).",
    oneOf: [
      {
        description: "Drawing objects.",
        type: "string",
        const: "drawingObjects",
      },
      {
        description: "Elements.",
        type: "string",
        const: "elements",
      },
      {
        description: "Element properties.",
        type: "string",
        const: "elementProperties",
      },
      {
        description: "Cells.",
        type: "string",
        const: "cells",
      },
      {
        description: "Keyframes.",
        type: "string",
        const: "keyframes",
      },
      {
        description: "A motion point.",
        type: "string",
        const: "motionPoint",
      },
      {
        description: "Library templates.",
        type: "string",
        const: "templates",
      },
      {
        description: "Function points.",
        type: "string",
        const: "functionPoints",
      },
      {
        description: "An image from the system clipboard.",
        type: "string",
        const: "image",
      },
    ],
  },
  "edit.copy.result": {
    title: "ClipResult",
    description: "`{clipboard: {kind, itemCount}}`.",
    type: "object",
    properties: {
      clipboard: {
        description: "What the clipboard holds now.",
        $ref: "#/$defs/edit.copy.ClipboardSummary",
      },
    },
    required: [
      "clipboard",
    ],
  },
  "edit.cut.EditContext": {
    description: "Selection / clipboard context (01 §6.2); `auto` follows the focused panel.",
    oneOf: [
      {
        description: "The focused panel's context.",
        type: "string",
        const: "auto",
      },
      {
        description: "Drawing objects of the current drawing.",
        type: "string",
        const: "drawing",
      },
      {
        description: "Elements in the camera/top/side views.",
        type: "string",
        const: "staging",
      },
      {
        description: "Exposure Sheet.",
        type: "string",
        const: "xsheet",
      },
      {
        description: "Timeline.",
        type: "string",
        const: "timeline",
      },
      {
        description: "Library.",
        type: "string",
        const: "library",
      },
      {
        description: "Function editor.",
        type: "string",
        const: "functionEditor",
      },
    ],
  },
  "edit.cut.params": {
    title: "ObjectsParams",
    description: "`context` + `objectIds` parameters of cut, copy and delete.",
    type: "object",
    properties: {
      context: {
        description: "Context; `auto` follows the focused panel.",
        $ref: "#/$defs/edit.cut.EditContext",
        default: "auto",
      },
      objectIds: {
        description: "Objects (default: the selection).",
        type: [
          "array",
          "null",
        ],
        items: {
          type: "string",
        },
        maxItems: 100000,
        default: null,
      },
    },
    additionalProperties: false,
  },
  "edit.cut.ClipboardSummary": {
    description: "`{kind, itemCount}`.",
    type: "object",
    properties: {
      kind: {
        description: "Kind.",
        $ref: "#/$defs/edit.cut.ClipboardKind",
      },
      itemCount: {
        description: "Items.",
        type: "integer",
        maximum: 2147483647,
        minimum: 0,
      },
    },
    required: [
      "kind",
      "itemCount",
    ],
  },
  "edit.cut.ClipboardKind": {
    description: "Clipboard content kinds (01 §12.4).",
    oneOf: [
      {
        description: "Drawing objects.",
        type: "string",
        const: "drawingObjects",
      },
      {
        description: "Elements.",
        type: "string",
        const: "elements",
      },
      {
        description: "Element properties.",
        type: "string",
        const: "elementProperties",
      },
      {
        description: "Cells.",
        type: "string",
        const: "cells",
      },
      {
        description: "Keyframes.",
        type: "string",
        const: "keyframes",
      },
      {
        description: "A motion point.",
        type: "string",
        const: "motionPoint",
      },
      {
        description: "Library templates.",
        type: "string",
        const: "templates",
      },
      {
        description: "Function points.",
        type: "string",
        const: "functionPoints",
      },
      {
        description: "An image from the system clipboard.",
        type: "string",
        const: "image",
      },
    ],
  },
  "edit.cut.result": {
    title: "ClipResult",
    description: "`{clipboard: {kind, itemCount}}`.",
    type: "object",
    properties: {
      clipboard: {
        description: "What the clipboard holds now.",
        $ref: "#/$defs/edit.cut.ClipboardSummary",
      },
    },
    required: [
      "clipboard",
    ],
  },
  "edit.cutSpecial.params": {
    title: "CutSpecialParams",
    description: "`edit.cutSpecial` parameters.",
    type: "object",
    properties: {
      objectIds: {
        description: "Objects (default: the selection).",
        type: [
          "array",
          "null",
        ],
        items: {
          type: "string",
        },
        default: null,
      },
      options: {
        description: "`TimelineEditOptions` (01 §12.4); omitted fields take the remembered values.",
        type: [
          "object",
          "null",
        ],
        additionalProperties: true,
        default: null,
      },
      remember: {
        description: "Remember the options.",
        type: "boolean",
        default: true,
      },
    },
    additionalProperties: false,
  },
  "edit.cutSpecial.ClipboardSummary": {
    description: "`{kind, itemCount}`.",
    type: "object",
    properties: {
      kind: {
        description: "Kind.",
        $ref: "#/$defs/edit.cutSpecial.ClipboardKind",
      },
      itemCount: {
        description: "Items.",
        type: "integer",
        maximum: 2147483647,
        minimum: 0,
      },
    },
    required: [
      "kind",
      "itemCount",
    ],
  },
  "edit.cutSpecial.ClipboardKind": {
    description: "Clipboard content kinds (01 §12.4).",
    oneOf: [
      {
        description: "Drawing objects.",
        type: "string",
        const: "drawingObjects",
      },
      {
        description: "Elements.",
        type: "string",
        const: "elements",
      },
      {
        description: "Element properties.",
        type: "string",
        const: "elementProperties",
      },
      {
        description: "Cells.",
        type: "string",
        const: "cells",
      },
      {
        description: "Keyframes.",
        type: "string",
        const: "keyframes",
      },
      {
        description: "A motion point.",
        type: "string",
        const: "motionPoint",
      },
      {
        description: "Library templates.",
        type: "string",
        const: "templates",
      },
      {
        description: "Function points.",
        type: "string",
        const: "functionPoints",
      },
      {
        description: "An image from the system clipboard.",
        type: "string",
        const: "image",
      },
    ],
  },
  "edit.cutSpecial.result": {
    title: "ClipResult",
    description: "`{clipboard: {kind, itemCount}}`.",
    type: "object",
    properties: {
      clipboard: {
        description: "What the clipboard holds now.",
        $ref: "#/$defs/edit.cutSpecial.ClipboardSummary",
      },
    },
    required: [
      "clipboard",
    ],
  },
  "edit.delete.EditContext": {
    description: "Selection / clipboard context (01 §6.2); `auto` follows the focused panel.",
    oneOf: [
      {
        description: "The focused panel's context.",
        type: "string",
        const: "auto",
      },
      {
        description: "Drawing objects of the current drawing.",
        type: "string",
        const: "drawing",
      },
      {
        description: "Elements in the camera/top/side views.",
        type: "string",
        const: "staging",
      },
      {
        description: "Exposure Sheet.",
        type: "string",
        const: "xsheet",
      },
      {
        description: "Timeline.",
        type: "string",
        const: "timeline",
      },
      {
        description: "Library.",
        type: "string",
        const: "library",
      },
      {
        description: "Function editor.",
        type: "string",
        const: "functionEditor",
      },
    ],
  },
  "edit.delete.params": {
    title: "ObjectsParams",
    description: "`context` + `objectIds` parameters of cut, copy and delete.",
    type: "object",
    properties: {
      context: {
        description: "Context; `auto` follows the focused panel.",
        $ref: "#/$defs/edit.delete.EditContext",
        default: "auto",
      },
      objectIds: {
        description: "Objects (default: the selection).",
        type: [
          "array",
          "null",
        ],
        items: {
          type: "string",
        },
        maxItems: 100000,
        default: null,
      },
    },
    additionalProperties: false,
  },
  "edit.delete.result": {
    title: "DeleteResult",
    description: "`{deletedIds}`.",
    type: "object",
    properties: {
      deletedIds: {
        description: "Deleted objects.",
        type: "array",
        items: {
          type: "string",
        },
      },
    },
    required: [
      "deletedIds",
    ],
  },
  "edit.deselectAll.EditContext": {
    description: "Selection / clipboard context (01 §6.2); `auto` follows the focused panel.",
    oneOf: [
      {
        description: "The focused panel's context.",
        type: "string",
        const: "auto",
      },
      {
        description: "Drawing objects of the current drawing.",
        type: "string",
        const: "drawing",
      },
      {
        description: "Elements in the camera/top/side views.",
        type: "string",
        const: "staging",
      },
      {
        description: "Exposure Sheet.",
        type: "string",
        const: "xsheet",
      },
      {
        description: "Timeline.",
        type: "string",
        const: "timeline",
      },
      {
        description: "Library.",
        type: "string",
        const: "library",
      },
      {
        description: "Function editor.",
        type: "string",
        const: "functionEditor",
      },
    ],
  },
  "edit.deselectAll.params": {
    title: "ContextParams",
    description: "`{context}` parameters.",
    type: "object",
    properties: {
      context: {
        description: "Selection context; `auto` follows the focused panel.",
        $ref: "#/$defs/edit.deselectAll.EditContext",
        default: "auto",
      },
    },
    additionalProperties: false,
  },
  "edit.deselectAll.result": {
    title: "ContextResult",
    description: "`{context}`.",
    type: "object",
    properties: {
      context: {
        description: "The resolved context.",
        type: "string",
      },
    },
    required: [
      "context",
    ],
  },
  "edit.history.changed.params": {
    title: "NoParams",
    description: "No parameters.",
    type: "object",
    additionalProperties: false,
  },
  "edit.history.changed.result": {
    title: "HistoryChanged",
    description: "`edit.history.changed` payload.",
    type: "object",
    properties: {
      position: {
        description: "Position.",
        type: "integer",
        format: "uint",
        minimum: 0,
      },
      entryCount: {
        description: "Entries.",
        type: "integer",
        format: "uint",
        minimum: 0,
      },
      lastLabel: {
        description: "Label of the top step.",
        type: [
          "string",
          "null",
        ],
      },
      source: {
        description: "`user` or `agent`.",
        type: [
          "string",
          "null",
        ],
      },
    },
    required: [
      "position",
      "entryCount",
    ],
  },
  "edit.history.get.params": {
    title: "HistoryGetParams",
    description: "`edit.history.get` parameters.",
    type: "object",
    properties: {
      offset: {
        description: "Entries to skip (from the oldest).",
        type: "integer",
        default: 0,
        minimum: 0,
        maximum: 100000,
      },
      limit: {
        description: "Entries to return.",
        type: "integer",
        default: 100,
        minimum: 1,
        maximum: 1000,
      },
    },
    additionalProperties: false,
  },
  "edit.history.get.HistoryEntry": {
    description: "One history entry (01 §12.1).",
    type: "object",
    properties: {
      index: {
        description: "1-based position of the step (position `n` = steps 1…n applied).",
        type: "integer",
        format: "uint",
        minimum: 0,
      },
      stepId: {
        description: "Step id (`u-<n>`), as in envelopes' `undo.stepId`.",
        type: "string",
      },
      label: {
        description: "Label.",
        type: "string",
      },
      source: {
        description: "`user` or `agent`.",
        type: "string",
      },
      agentSessionId: {
        description: "Agent session id.",
        type: [
          "string",
          "null",
        ],
      },
      agentName: {
        description: "Agent display name.",
        type: [
          "string",
          "null",
        ],
      },
      time: {
        description: "Commit time (ISO 8601 UTC).",
        type: "string",
      },
      objectCount: {
        description: "Changed objects.",
        type: "integer",
        format: "uint",
        minimum: 0,
      },
    },
    required: [
      "index",
      "stepId",
      "label",
      "source",
      "time",
      "objectCount",
    ],
  },
  "edit.history.get.result": {
    title: "HistoryGetResult",
    description: "`edit.history.get` result.",
    type: "object",
    properties: {
      entries: {
        description: "Entries, oldest first.",
        type: "array",
        items: {
          $ref: "#/$defs/edit.history.get.HistoryEntry",
        },
      },
      position: {
        description: "Steps applied.",
        type: "integer",
        format: "uint",
        minimum: 0,
      },
      cleanPosition: {
        description: "Position of the last save (`null` if unreachable).",
        type: [
          "integer",
          "null",
        ],
        format: "uint",
        minimum: 0,
      },
      total: {
        description: "Total entries.",
        type: "integer",
        format: "uint",
        minimum: 0,
      },
    },
    required: [
      "entries",
      "position",
      "total",
    ],
  },
  "edit.history.move.params": {
    title: "HistoryMoveParams",
    description: "`edit.history.move` parameters.",
    type: "object",
    properties: {
      position: {
        description: "Target position (0 = before the first step, entry count = after the last).",
        type: "integer",
        maximum: 1000,
        minimum: 0,
      },
    },
    additionalProperties: false,
    required: [
      "position",
    ],
  },
  "edit.history.move.result": {
    title: "PositionResult",
    description: "`{position}`.",
    type: "object",
    properties: {
      position: {
        description: "History position after the call.",
        type: "integer",
        format: "uint",
        minimum: 0,
      },
    },
    required: [
      "position",
    ],
  },
  "edit.paste.EditContext": {
    description: "Selection / clipboard context (01 §6.2); `auto` follows the focused panel.",
    oneOf: [
      {
        description: "The focused panel's context.",
        type: "string",
        const: "auto",
      },
      {
        description: "Drawing objects of the current drawing.",
        type: "string",
        const: "drawing",
      },
      {
        description: "Elements in the camera/top/side views.",
        type: "string",
        const: "staging",
      },
      {
        description: "Exposure Sheet.",
        type: "string",
        const: "xsheet",
      },
      {
        description: "Timeline.",
        type: "string",
        const: "timeline",
      },
      {
        description: "Library.",
        type: "string",
        const: "library",
      },
      {
        description: "Function editor.",
        type: "string",
        const: "functionEditor",
      },
    ],
  },
  "edit.paste.params": {
    title: "PasteParams",
    description: "`edit.paste` parameters.",
    type: "object",
    properties: {
      context: {
        description: "Context; `auto` follows the focused panel.",
        $ref: "#/$defs/edit.paste.EditContext",
        default: "auto",
      },
      destination: {
        description: "Where to paste: `{drawingId}`, `{elementId, frame}`, `{catalogId}`, `{functionId, frame}` or `{objectIds}`.",
        type: [
          "object",
          "null",
        ],
        additionalProperties: true,
        default: null,
      },
    },
    additionalProperties: false,
  },
  "edit.paste.Warning": {
    description: "A warning (11 §6.3, §10): same code space as errors, never aborts.",
    type: "object",
    properties: {
      code: {
        description: "Code.",
        type: "string",
      },
      message: {
        description: "Message.",
        type: "string",
      },
      objectIds: {
        description: "Affected objects.",
        type: "array",
        items: {
          type: "string",
        },
      },
    },
    required: [
      "code",
      "message",
    ],
  },
  "edit.paste.result": {
    title: "PasteResult",
    description: "Paste result.",
    type: "object",
    properties: {
      createdIds: {
        description: "Created objects.",
        type: "array",
        items: {
          type: "string",
        },
      },
      modifiedIds: {
        description: "Modified objects.",
        type: "array",
        items: {
          type: "string",
        },
      },
      warnings: {
        description: "Warnings.",
        type: "array",
        items: {
          $ref: "#/$defs/edit.paste.Warning",
        },
      },
    },
    required: [
      "createdIds",
      "modifiedIds",
      "warnings",
    ],
  },
  "edit.pasteAsNewElement.params": {
    title: "PasteNewParams",
    description: "`edit.pasteAsNewElement` parameters.",
    type: "object",
    properties: {
      sceneId: {
        description: "Scene (default current).",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
      frame: {
        description: "Frame (default current).",
        type: [
          "integer",
          "null",
        ],
        default: null,
        minimum: 1,
        maximum: 100000,
      },
    },
    additionalProperties: false,
  },
  "edit.pasteAsNewElement.result": {
    title: "NewElementResult",
    description: "`{elementId}`.",
    type: "object",
    properties: {
      elementId: {
        description: "The new element.",
        type: "string",
      },
    },
    required: [
      "elementId",
    ],
  },
  "edit.pasteSpecial.params": {
    title: "PasteSpecialParams",
    description: "`edit.pasteSpecial` parameters.",
    type: "object",
    properties: {
      destination: {
        description: "Where to paste.",
        type: [
          "object",
          "null",
        ],
        additionalProperties: true,
        default: null,
      },
      options: {
        description: "`TimelineEditOptions` or `MotionPointPasteOptions` (01 §12.4).",
        type: [
          "object",
          "null",
        ],
        additionalProperties: true,
        default: null,
      },
      useLast: {
        description: "Use the last options.",
        type: "boolean",
        default: false,
      },
      remember: {
        description: "Remember the options.",
        type: "boolean",
        default: true,
      },
    },
    additionalProperties: false,
  },
  "edit.pasteSpecial.Warning": {
    description: "A warning (11 §6.3, §10): same code space as errors, never aborts.",
    type: "object",
    properties: {
      code: {
        description: "Code.",
        type: "string",
      },
      message: {
        description: "Message.",
        type: "string",
      },
      objectIds: {
        description: "Affected objects.",
        type: "array",
        items: {
          type: "string",
        },
      },
    },
    required: [
      "code",
      "message",
    ],
  },
  "edit.pasteSpecial.result": {
    title: "PasteResult",
    description: "Paste result.",
    type: "object",
    properties: {
      createdIds: {
        description: "Created objects.",
        type: "array",
        items: {
          type: "string",
        },
      },
      modifiedIds: {
        description: "Modified objects.",
        type: "array",
        items: {
          type: "string",
        },
      },
      warnings: {
        description: "Warnings.",
        type: "array",
        items: {
          $ref: "#/$defs/edit.pasteSpecial.Warning",
        },
      },
    },
    required: [
      "createdIds",
      "modifiedIds",
      "warnings",
    ],
  },
  "edit.redo.params": {
    title: "RedoParams",
    description: "`edit.redo` parameters.",
    type: "object",
    properties: {
      steps: {
        description: "Steps to redo.",
        type: "integer",
        default: 1,
        minimum: 1,
        maximum: 1000,
      },
    },
    additionalProperties: false,
  },
  "edit.redo.result": {
    title: "RedoResult",
    description: "`edit.redo` result.",
    type: "object",
    properties: {
      redone: {
        description: "Labels of the redone steps, in order.",
        type: "array",
        items: {
          type: "string",
        },
      },
      position: {
        description: "History position after the call.",
        type: "integer",
        format: "uint",
        minimum: 0,
      },
    },
    required: [
      "redone",
      "position",
    ],
  },
  "edit.selectAll.EditContext": {
    description: "Selection / clipboard context (01 §6.2); `auto` follows the focused panel.",
    oneOf: [
      {
        description: "The focused panel's context.",
        type: "string",
        const: "auto",
      },
      {
        description: "Drawing objects of the current drawing.",
        type: "string",
        const: "drawing",
      },
      {
        description: "Elements in the camera/top/side views.",
        type: "string",
        const: "staging",
      },
      {
        description: "Exposure Sheet.",
        type: "string",
        const: "xsheet",
      },
      {
        description: "Timeline.",
        type: "string",
        const: "timeline",
      },
      {
        description: "Library.",
        type: "string",
        const: "library",
      },
      {
        description: "Function editor.",
        type: "string",
        const: "functionEditor",
      },
    ],
  },
  "edit.selectAll.params": {
    title: "ContextParams",
    description: "`{context}` parameters.",
    type: "object",
    properties: {
      context: {
        description: "Selection context; `auto` follows the focused panel.",
        $ref: "#/$defs/edit.selectAll.EditContext",
        default: "auto",
      },
    },
    additionalProperties: false,
  },
  "edit.selectAll.result": {
    title: "SelectAllResult",
    description: "`edit.selectAll` result.",
    type: "object",
    properties: {
      context: {
        description: "The resolved context.",
        type: "string",
      },
      ids: {
        description: "The selection.",
        type: "array",
        items: {
          type: "string",
        },
      },
    },
    required: [
      "context",
      "ids",
    ],
  },
  "edit.selection.changed.params": {
    title: "NoParams",
    description: "No parameters.",
    type: "object",
    additionalProperties: false,
  },
  "edit.selection.changed.result": {
    title: "SelectionResult",
    description: "`{context, ids}`.",
    type: "object",
    properties: {
      context: {
        description: "The resolved context.",
        type: "string",
      },
      ids: {
        description: "The selection.",
        type: "array",
        items: {
          type: "string",
        },
      },
    },
    required: [
      "context",
      "ids",
    ],
  },
  "edit.selection.get.EditContext": {
    description: "Selection / clipboard context (01 §6.2); `auto` follows the focused panel.",
    oneOf: [
      {
        description: "The focused panel's context.",
        type: "string",
        const: "auto",
      },
      {
        description: "Drawing objects of the current drawing.",
        type: "string",
        const: "drawing",
      },
      {
        description: "Elements in the camera/top/side views.",
        type: "string",
        const: "staging",
      },
      {
        description: "Exposure Sheet.",
        type: "string",
        const: "xsheet",
      },
      {
        description: "Timeline.",
        type: "string",
        const: "timeline",
      },
      {
        description: "Library.",
        type: "string",
        const: "library",
      },
      {
        description: "Function editor.",
        type: "string",
        const: "functionEditor",
      },
    ],
  },
  "edit.selection.get.params": {
    title: "ContextParams",
    description: "`{context}` parameters.",
    type: "object",
    properties: {
      context: {
        description: "Selection context; `auto` follows the focused panel.",
        $ref: "#/$defs/edit.selection.get.EditContext",
        default: "auto",
      },
    },
    additionalProperties: false,
  },
  "edit.selection.get.result": {
    title: "SelectionGetResult",
    description: "`edit.selection.get` result.",
    type: "object",
    properties: {
      context: {
        description: "The resolved context.",
        type: "string",
      },
      ids: {
        description: "Selected ids.",
        type: "array",
        items: {
          type: "string",
        },
      },
      kinds: {
        description: "Object types present.",
        type: "array",
        items: {
          type: "string",
        },
      },
    },
    required: [
      "context",
      "ids",
      "kinds",
    ],
  },
  "edit.selection.set.EditContext": {
    description: "Selection / clipboard context (01 §6.2); `auto` follows the focused panel.",
    oneOf: [
      {
        description: "The focused panel's context.",
        type: "string",
        const: "auto",
      },
      {
        description: "Drawing objects of the current drawing.",
        type: "string",
        const: "drawing",
      },
      {
        description: "Elements in the camera/top/side views.",
        type: "string",
        const: "staging",
      },
      {
        description: "Exposure Sheet.",
        type: "string",
        const: "xsheet",
      },
      {
        description: "Timeline.",
        type: "string",
        const: "timeline",
      },
      {
        description: "Library.",
        type: "string",
        const: "library",
      },
      {
        description: "Function editor.",
        type: "string",
        const: "functionEditor",
      },
    ],
  },
  "edit.selection.set.SelectMode": {
    description: "Selection change mode.",
    oneOf: [
      {
        description: "Replace the selection.",
        type: "string",
        const: "replace",
      },
      {
        description: "Add to it.",
        type: "string",
        const: "add",
      },
      {
        description: "Remove from it.",
        type: "string",
        const: "remove",
      },
      {
        description: "Toggle membership.",
        type: "string",
        const: "toggle",
      },
    ],
  },
  "edit.selection.set.params": {
    title: "SelectionSetParams",
    description: "`edit.selection.set` parameters.",
    type: "object",
    properties: {
      context: {
        description: "Selection context.",
        $ref: "#/$defs/edit.selection.set.EditContext",
        default: "auto",
      },
      ids: {
        description: "Objects (ids or paths).",
        type: "array",
        items: {
          type: "string",
        },
        maxItems: 100000,
      },
      mode: {
        description: "How to combine with the current selection.",
        $ref: "#/$defs/edit.selection.set.SelectMode",
      },
    },
    additionalProperties: false,
    required: [
      "ids",
    ],
  },
  "edit.selection.set.result": {
    title: "SelectionResult",
    description: "`{context, ids}`.",
    type: "object",
    properties: {
      context: {
        description: "The resolved context.",
        type: "string",
      },
      ids: {
        description: "The selection.",
        type: "array",
        items: {
          type: "string",
        },
      },
    },
    required: [
      "context",
      "ids",
    ],
  },
  "edit.undo.params": {
    title: "UndoParams",
    description: "`edit.undo` parameters.",
    type: "object",
    properties: {
      steps: {
        description: "Steps to undo (each is its own transaction).",
        type: "integer",
        default: 1,
        minimum: 1,
        maximum: 1000,
      },
      ifTop: {
        description: "Undo only if this step id is still the most recent one (`undo.notTop` otherwise; 11 §7.7).",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
    },
    additionalProperties: false,
  },
  "edit.undo.result": {
    title: "UndoResult",
    description: "`edit.undo` result.",
    type: "object",
    properties: {
      undone: {
        description: "Labels of the undone steps, most recent first.",
        type: "array",
        items: {
          type: "string",
        },
      },
      position: {
        description: "History position after the call.",
        type: "integer",
        format: "uint",
        minimum: 0,
      },
    },
    required: [
      "undone",
      "position",
    ],
  },
  "inspect.contactSheet.render.params": {
    title: "ContactSheetParams",
    description: "`inspect.contactSheet.render` parameters.",
    type: "object",
    properties: {
      sceneId: {
        description: "Scene.",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
      frames: {
        description: "Frames: a list, or `{from, to, step}`.",
      },
      columns: {
        description: "Columns.",
        type: [
          "integer",
          "null",
        ],
        default: null,
        minimum: 1,
        maximum: 64,
      },
      cellWidth: {
        description: "Cell width.",
        type: [
          "integer",
          "null",
        ],
        default: null,
        minimum: 16,
        maximum: 2048,
      },
      labels: {
        description: "Frame labels.",
        type: [
          "boolean",
          "null",
        ],
        default: null,
      },
    },
    additionalProperties: false,
    required: [
      "frames",
    ],
  },
  "inspect.contactSheet.render.result": {
    title: "Map_of_AnyValue",
    type: "object",
    additionalProperties: true,
  },
  "inspect.coordinates.convert.Point3": {
    description: "A point with an optional depth.",
    type: "object",
    properties: {
      x: {
        description: "X.",
        type: "number",
      },
      y: {
        description: "Y.",
        type: "number",
      },
      z: {
        description: "Z.",
        type: [
          "number",
          "null",
        ],
      },
    },
    additionalProperties: false,
    required: [
      "x",
      "y",
    ],
  },
  "inspect.coordinates.convert.params": {
    title: "ConvertParams",
    description: "`inspect.coordinates.convert` parameters.",
    type: "object",
    properties: {
      from: {
        description: "Source space.",
        type: "string",
      },
      to: {
        description: "Target space.",
        type: "string",
      },
      points: {
        description: "Points.",
        type: "array",
        items: {
          $ref: "#/$defs/inspect.coordinates.convert.Point3",
        },
        minItems: 1,
        maxItems: 100000,
      },
      sceneId: {
        description: "Scene.",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
      frame: {
        description: "Frame.",
        type: [
          "integer",
          "null",
        ],
        default: null,
        minimum: 1,
        maximum: 100000,
      },
      elementId: {
        description: "Element.",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
      view: {
        description: "View.",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
    },
    additionalProperties: false,
    required: [
      "from",
      "to",
      "points",
    ],
  },
  "inspect.coordinates.convert.result": {
    title: "Points",
    description: "`{points}`.",
    type: "object",
    properties: {
      points: {
        description: "Converted points.",
        type: "array",
        items: {
          $ref: "#/$defs/inspect.coordinates.convert.Point3",
        },
      },
    },
    required: [
      "points",
    ],
  },
  "inspect.frame.describe.Detail": {
    description: "Description detail.",
    oneOf: [
      {
        description: "Summary.",
        type: "string",
        const: "summary",
      },
      {
        description: "Everything.",
        type: "string",
        const: "full",
      },
    ],
  },
  "inspect.frame.describe.params": {
    title: "FrameParams",
    description: "`inspect.frame.describe` parameters.",
    type: "object",
    properties: {
      sceneId: {
        description: "Scene (default current).",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
      frame: {
        description: "Frame (default current).",
        type: [
          "integer",
          "null",
        ],
        default: null,
        minimum: 1,
        maximum: 100000,
      },
      cameraId: {
        description: "Camera (default the scene's current camera).",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
      detail: {
        description: "Detail.",
        $ref: "#/$defs/inspect.frame.describe.Detail",
      },
    },
    additionalProperties: false,
  },
  "inspect.frame.describe.result": {
    title: "Map_of_AnyValue",
    type: "object",
    additionalProperties: true,
  },
  "inspect.hitTest.Space": {
    description: "Coordinate space.",
    oneOf: [
      {
        description: "View pixels.",
        type: "string",
        const: "view",
      },
      {
        description: "Drawing space.",
        type: "string",
        const: "drawing",
      },
      {
        description: "Scene space.",
        type: "string",
        const: "scene",
      },
      {
        description: "Camera space.",
        type: "string",
        const: "camera",
      },
      {
        description: "Rendered image pixels.",
        type: "string",
        const: "image",
      },
    ],
  },
  "inspect.hitTest.Point": {
    description: "A 2-D point.",
    type: "object",
    properties: {
      x: {
        description: "X.",
        type: "number",
      },
      y: {
        description: "Y.",
        type: "number",
      },
    },
    additionalProperties: false,
    required: [
      "x",
      "y",
    ],
  },
  "inspect.hitTest.params": {
    title: "HitParams",
    description: "`inspect.hitTest` parameters.",
    type: "object",
    properties: {
      space: {
        description: "Space of `point`.",
        $ref: "#/$defs/inspect.hitTest.Space",
      },
      point: {
        description: "The point.",
        $ref: "#/$defs/inspect.hitTest.Point",
      },
      sceneId: {
        description: "Scene.",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
      frame: {
        description: "Frame.",
        type: [
          "integer",
          "null",
        ],
        default: null,
        minimum: 1,
        maximum: 100000,
      },
      view: {
        description: "View.",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
    },
    additionalProperties: false,
    required: [
      "space",
      "point",
    ],
  },
  "inspect.hitTest.result": {
    title: "Hits",
    description: "`{hits}`.",
    type: "object",
    properties: {
      hits: {
        description: "Hits, front to back.",
        type: "array",
        items: {
          type: "object",
          additionalProperties: true,
        },
      },
    },
    required: [
      "hits",
    ],
  },
  "inspect.screenshot.Rect": {
    description: "A rectangle.",
    type: "object",
    properties: {
      x: {
        description: "Left.",
        type: "number",
      },
      y: {
        description: "Top.",
        type: "number",
      },
      w: {
        description: "Width.",
        type: "number",
      },
      h: {
        description: "Height.",
        type: "number",
      },
    },
    additionalProperties: false,
    required: [
      "x",
      "y",
      "w",
      "h",
    ],
  },
  "inspect.screenshot.params": {
    title: "ScreenshotParams",
    description: "`inspect.screenshot` parameters.",
    type: "object",
    properties: {
      target: {
        description: "Window, view, panel or dialog.",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
      region: {
        description: "Region.",
        anyOf: [
          {
            $ref: "#/$defs/inspect.screenshot.Rect",
          },
          {
            type: "null",
          },
        ],
        default: null,
      },
    },
    additionalProperties: false,
  },
  "inspect.screenshot.result": {
    title: "Map_of_AnyValue",
    type: "object",
    additionalProperties: true,
  },
  "inspect.ui.tree.params": {
    title: "UiTreeParams",
    description: "`inspect.ui.tree` parameters.",
    type: "object",
    properties: {
      target: {
        description: "Window, panel or dialog.",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
      depth: {
        description: "Depth.",
        type: [
          "integer",
          "null",
        ],
        default: null,
        minimum: 1,
        maximum: 32,
      },
    },
    additionalProperties: false,
  },
  "inspect.ui.tree.result": {
    title: "Map_of_AnyValue",
    type: "object",
    additionalProperties: true,
  },
  "job.cancel.params": {
    title: "JobIdParams",
    description: "`{jobId}` parameters.",
    type: "object",
    properties: {
      jobId: {
        description: "The job.",
        type: "string",
      },
    },
    additionalProperties: false,
    required: [
      "jobId",
    ],
  },
  "job.cancel.JobState": {
    description: "Job state (11 §7.7).",
    oneOf: [
      {
        description: "Accepted, not started.",
        type: "string",
        const: "queued",
      },
      {
        description: "Running.",
        type: "string",
        const: "running",
      },
      {
        description: "Finished with a result.",
        type: "string",
        const: "succeeded",
      },
      {
        description: "Finished with an error.",
        type: "string",
        const: "failed",
      },
      {
        description: "Cancelled.",
        type: "string",
        const: "cancelled",
      },
    ],
  },
  "job.cancel.CommandError": {
    description: "A typed command error in the `{code, message, details, hint?}` wire form (11 §10).",
    type: "object",
    properties: {
      code: {
        description: "Stable dotted code `area.reason`.",
        type: "string",
      },
      message: {
        description: "Human-readable message.",
        type: "string",
      },
      details: {
        description: "Structured details (boxed: the error travels in every `Result` of the command path, so it stays small).",
      },
      hint: {
        description: "How to fix the call, where known.",
        type: [
          "string",
          "null",
        ],
      },
    },
    required: [
      "code",
      "message",
      "details",
    ],
  },
  "job.cancel.result": {
    title: "JobRecord",
    description: "A job record (11 §7.7 `job_status`).",
    type: "object",
    properties: {
      jobId: {
        description: "Job id.",
        type: "string",
      },
      commandId: {
        description: "The job command.",
        type: "string",
      },
      state: {
        description: "State.",
        $ref: "#/$defs/job.cancel.JobState",
      },
      progress: {
        description: "Progress 0..1.",
        type: "number",
      },
      message: {
        description: "Current step text.",
        type: "string",
      },
      startedAt: {
        description: "Start time (ISO 8601 UTC).",
        type: "string",
      },
      result: {
        description: "Final result.",
      },
      error: {
        description: "Final error.",
        anyOf: [
          {
            $ref: "#/$defs/job.cancel.CommandError",
          },
          {
            type: "null",
          },
        ],
      },
      session: {
        description: "Session that started it.",
        type: "string",
      },
    },
    required: [
      "jobId",
      "commandId",
      "state",
      "progress",
      "message",
      "startedAt",
      "session",
    ],
  },
  "job.finished.params": {
    title: "NoParams",
    description: "No parameters.",
    type: "object",
    additionalProperties: false,
  },
  "job.finished.JobState": {
    description: "Job state (11 §7.7).",
    oneOf: [
      {
        description: "Accepted, not started.",
        type: "string",
        const: "queued",
      },
      {
        description: "Running.",
        type: "string",
        const: "running",
      },
      {
        description: "Finished with a result.",
        type: "string",
        const: "succeeded",
      },
      {
        description: "Finished with an error.",
        type: "string",
        const: "failed",
      },
      {
        description: "Cancelled.",
        type: "string",
        const: "cancelled",
      },
    ],
  },
  "job.finished.CommandError": {
    description: "A typed command error in the `{code, message, details, hint?}` wire form (11 §10).",
    type: "object",
    properties: {
      code: {
        description: "Stable dotted code `area.reason`.",
        type: "string",
      },
      message: {
        description: "Human-readable message.",
        type: "string",
      },
      details: {
        description: "Structured details (boxed: the error travels in every `Result` of the command path, so it stays small).",
      },
      hint: {
        description: "How to fix the call, where known.",
        type: [
          "string",
          "null",
        ],
      },
    },
    required: [
      "code",
      "message",
      "details",
    ],
  },
  "job.finished.result": {
    title: "JobRecord",
    description: "A job record (11 §7.7 `job_status`).",
    type: "object",
    properties: {
      jobId: {
        description: "Job id.",
        type: "string",
      },
      commandId: {
        description: "The job command.",
        type: "string",
      },
      state: {
        description: "State.",
        $ref: "#/$defs/job.finished.JobState",
      },
      progress: {
        description: "Progress 0..1.",
        type: "number",
      },
      message: {
        description: "Current step text.",
        type: "string",
      },
      startedAt: {
        description: "Start time (ISO 8601 UTC).",
        type: "string",
      },
      result: {
        description: "Final result.",
      },
      error: {
        description: "Final error.",
        anyOf: [
          {
            $ref: "#/$defs/job.finished.CommandError",
          },
          {
            type: "null",
          },
        ],
      },
      session: {
        description: "Session that started it.",
        type: "string",
      },
    },
    required: [
      "jobId",
      "commandId",
      "state",
      "progress",
      "message",
      "startedAt",
      "session",
    ],
  },
  "job.list.params": {
    title: "ListParams",
    description: "`job.list` parameters.",
    type: "object",
    properties: {
      jobId: {
        description: "One job.",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
      all: {
        description: "Every session's jobs (default: the caller's own).",
        type: "boolean",
        default: false,
      },
    },
    additionalProperties: false,
  },
  "job.list.JobRecord": {
    description: "A job record (11 §7.7 `job_status`).",
    type: "object",
    properties: {
      jobId: {
        description: "Job id.",
        type: "string",
      },
      commandId: {
        description: "The job command.",
        type: "string",
      },
      state: {
        description: "State.",
        $ref: "#/$defs/job.list.JobState",
      },
      progress: {
        description: "Progress 0..1.",
        type: "number",
      },
      message: {
        description: "Current step text.",
        type: "string",
      },
      startedAt: {
        description: "Start time (ISO 8601 UTC).",
        type: "string",
      },
      result: {
        description: "Final result.",
      },
      error: {
        description: "Final error.",
        anyOf: [
          {
            $ref: "#/$defs/job.list.CommandError",
          },
          {
            type: "null",
          },
        ],
      },
      session: {
        description: "Session that started it.",
        type: "string",
      },
    },
    required: [
      "jobId",
      "commandId",
      "state",
      "progress",
      "message",
      "startedAt",
      "session",
    ],
  },
  "job.list.JobState": {
    description: "Job state (11 §7.7).",
    oneOf: [
      {
        description: "Accepted, not started.",
        type: "string",
        const: "queued",
      },
      {
        description: "Running.",
        type: "string",
        const: "running",
      },
      {
        description: "Finished with a result.",
        type: "string",
        const: "succeeded",
      },
      {
        description: "Finished with an error.",
        type: "string",
        const: "failed",
      },
      {
        description: "Cancelled.",
        type: "string",
        const: "cancelled",
      },
    ],
  },
  "job.list.CommandError": {
    description: "A typed command error in the `{code, message, details, hint?}` wire form (11 §10).",
    type: "object",
    properties: {
      code: {
        description: "Stable dotted code `area.reason`.",
        type: "string",
      },
      message: {
        description: "Human-readable message.",
        type: "string",
      },
      details: {
        description: "Structured details (boxed: the error travels in every `Result` of the command path, so it stays small).",
      },
      hint: {
        description: "How to fix the call, where known.",
        type: [
          "string",
          "null",
        ],
      },
    },
    required: [
      "code",
      "message",
      "details",
    ],
  },
  "job.list.result": {
    title: "Array_of_JobRecord",
    type: "array",
    items: {
      $ref: "#/$defs/job.list.JobRecord",
    },
  },
  "job.progress.params": {
    title: "NoParams",
    description: "No parameters.",
    type: "object",
    additionalProperties: false,
  },
  "job.progress.JobState": {
    description: "Job state (11 §7.7).",
    oneOf: [
      {
        description: "Accepted, not started.",
        type: "string",
        const: "queued",
      },
      {
        description: "Running.",
        type: "string",
        const: "running",
      },
      {
        description: "Finished with a result.",
        type: "string",
        const: "succeeded",
      },
      {
        description: "Finished with an error.",
        type: "string",
        const: "failed",
      },
      {
        description: "Cancelled.",
        type: "string",
        const: "cancelled",
      },
    ],
  },
  "job.progress.CommandError": {
    description: "A typed command error in the `{code, message, details, hint?}` wire form (11 §10).",
    type: "object",
    properties: {
      code: {
        description: "Stable dotted code `area.reason`.",
        type: "string",
      },
      message: {
        description: "Human-readable message.",
        type: "string",
      },
      details: {
        description: "Structured details (boxed: the error travels in every `Result` of the command path, so it stays small).",
      },
      hint: {
        description: "How to fix the call, where known.",
        type: [
          "string",
          "null",
        ],
      },
    },
    required: [
      "code",
      "message",
      "details",
    ],
  },
  "job.progress.result": {
    title: "JobRecord",
    description: "A job record (11 §7.7 `job_status`).",
    type: "object",
    properties: {
      jobId: {
        description: "Job id.",
        type: "string",
      },
      commandId: {
        description: "The job command.",
        type: "string",
      },
      state: {
        description: "State.",
        $ref: "#/$defs/job.progress.JobState",
      },
      progress: {
        description: "Progress 0..1.",
        type: "number",
      },
      message: {
        description: "Current step text.",
        type: "string",
      },
      startedAt: {
        description: "Start time (ISO 8601 UTC).",
        type: "string",
      },
      result: {
        description: "Final result.",
      },
      error: {
        description: "Final error.",
        anyOf: [
          {
            $ref: "#/$defs/job.progress.CommandError",
          },
          {
            type: "null",
          },
        ],
      },
      session: {
        description: "Session that started it.",
        type: "string",
      },
    },
    required: [
      "jobId",
      "commandId",
      "state",
      "progress",
      "message",
      "startedAt",
      "session",
    ],
  },
  "job.wait.params": {
    title: "WaitParams",
    description: "`job.wait` parameters.",
    type: "object",
    properties: {
      jobId: {
        description: "The job.",
        type: "string",
      },
      timeoutMs: {
        description: "How long to wait.",
        type: "integer",
        default: 60000,
        minimum: 0,
        maximum: 3600000,
      },
    },
    additionalProperties: false,
    required: [
      "jobId",
    ],
  },
  "job.wait.JobState": {
    description: "Job state (11 §7.7).",
    oneOf: [
      {
        description: "Accepted, not started.",
        type: "string",
        const: "queued",
      },
      {
        description: "Running.",
        type: "string",
        const: "running",
      },
      {
        description: "Finished with a result.",
        type: "string",
        const: "succeeded",
      },
      {
        description: "Finished with an error.",
        type: "string",
        const: "failed",
      },
      {
        description: "Cancelled.",
        type: "string",
        const: "cancelled",
      },
    ],
  },
  "job.wait.CommandError": {
    description: "A typed command error in the `{code, message, details, hint?}` wire form (11 §10).",
    type: "object",
    properties: {
      code: {
        description: "Stable dotted code `area.reason`.",
        type: "string",
      },
      message: {
        description: "Human-readable message.",
        type: "string",
      },
      details: {
        description: "Structured details (boxed: the error travels in every `Result` of the command path, so it stays small).",
      },
      hint: {
        description: "How to fix the call, where known.",
        type: [
          "string",
          "null",
        ],
      },
    },
    required: [
      "code",
      "message",
      "details",
    ],
  },
  "job.wait.result": {
    title: "WaitResult",
    description: "`job.wait` result: the job record and whether the wait timed out.",
    type: "object",
    properties: {
      jobId: {
        description: "Job id.",
        type: "string",
      },
      commandId: {
        description: "The job command.",
        type: "string",
      },
      state: {
        description: "State.",
        $ref: "#/$defs/job.wait.JobState",
      },
      progress: {
        description: "Progress 0..1.",
        type: "number",
      },
      message: {
        description: "Current step text.",
        type: "string",
      },
      startedAt: {
        description: "Start time (ISO 8601 UTC).",
        type: "string",
      },
      result: {
        description: "Final result.",
      },
      error: {
        description: "Final error.",
        anyOf: [
          {
            $ref: "#/$defs/job.wait.CommandError",
          },
          {
            type: "null",
          },
        ],
      },
      session: {
        description: "Session that started it.",
        type: "string",
      },
      timedOut: {
        description: "The job was still running when the timeout passed.",
        type: "boolean",
      },
    },
    required: [
      "jobId",
      "commandId",
      "state",
      "progress",
      "message",
      "startedAt",
      "session",
      "timedOut",
    ],
  },
  "project.autosaved.params": {
    title: "NoParams",
    description: "No parameters.",
    type: "object",
    additionalProperties: false,
  },
  "project.autosaved.result": {
    title: "Map_of_AnyValue",
    type: "object",
    additionalProperties: true,
  },
  "project.backup.list.params": {
    title: "BackupListParams",
    description: "`project.backup.list` parameters.",
    type: "object",
    properties: {
      path: {
        description: "Project path (default: the current project).",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
    },
    additionalProperties: false,
  },
  "project.backup.list.result": {
    title: "Array_of_Map_of_AnyValue",
    type: "array",
    items: {
      type: "object",
      additionalProperties: true,
    },
  },
  "project.backup.open.OnUnsaved": {
    description: "What to do with unsaved changes (01 §11.2 rule 3).",
    oneOf: [
      {
        description: "Fail with `project.unsavedChanges` when something is modified.",
        type: "string",
        const: "fail",
      },
      {
        description: "Save first.",
        type: "string",
        const: "save",
      },
      {
        description: "Discard the changes.",
        type: "string",
        const: "discard",
      },
    ],
  },
  "project.backup.open.params": {
    title: "BackupOpenParams",
    description: "`project.backup.open` parameters.",
    type: "object",
    properties: {
      backupId: {
        description: "Backup id.",
        type: "string",
      },
      onUnsaved: {
        description: "Unsaved-changes policy.",
        $ref: "#/$defs/project.backup.open.OnUnsaved",
      },
    },
    additionalProperties: false,
    required: [
      "backupId",
    ],
  },
  "project.backup.open.result": {
    title: "ProjectId",
    description: "`{projectId}`.",
    type: "object",
    properties: {
      projectId: {
        description: "Project id.",
        type: "string",
      },
    },
    required: [
      "projectId",
    ],
  },
  "project.close.OnUnsaved": {
    description: "What to do with unsaved changes (01 §11.2 rule 3).",
    oneOf: [
      {
        description: "Fail with `project.unsavedChanges` when something is modified.",
        type: "string",
        const: "fail",
      },
      {
        description: "Save first.",
        type: "string",
        const: "save",
      },
      {
        description: "Discard the changes.",
        type: "string",
        const: "discard",
      },
    ],
  },
  "project.close.params": {
    title: "UnsavedParams",
    description: "`{onUnsaved}` parameters.",
    type: "object",
    properties: {
      onUnsaved: {
        description: "Unsaved-changes policy.",
        $ref: "#/$defs/project.close.OnUnsaved",
      },
    },
    additionalProperties: false,
  },
  "project.close.result": {
    title: "ClosedResult",
    description: "`{closed}`.",
    type: "object",
    properties: {
      closed: {
        description: "The project was closed.",
        type: "boolean",
      },
    },
    required: [
      "closed",
    ],
  },
  "project.closed.params": {
    title: "NoParams",
    description: "No parameters.",
    type: "object",
    additionalProperties: false,
  },
  "project.closed.result": {
    title: "ProjectId",
    description: "`{projectId}`.",
    type: "object",
    properties: {
      projectId: {
        description: "Project id.",
        type: "string",
      },
    },
    required: [
      "projectId",
    ],
  },
  "project.create.OnUnsaved": {
    description: "What to do with unsaved changes (01 §11.2 rule 3).",
    oneOf: [
      {
        description: "Fail with `project.unsavedChanges` when something is modified.",
        type: "string",
        const: "fail",
      },
      {
        description: "Save first.",
        type: "string",
        const: "save",
      },
      {
        description: "Discard the changes.",
        type: "string",
        const: "discard",
      },
    ],
  },
  "project.create.OnExists": {
    description: "What to do when a target file exists (01 §11.2 rule 4).",
    oneOf: [
      {
        description: "Fail with `fs.exists` / `project.exists`.",
        type: "string",
        const: "fail",
      },
      {
        description: "Overwrite.",
        type: "string",
        const: "overwrite",
      },
    ],
  },
  "project.create.params": {
    title: "CreateParams",
    description: "`project.create` parameters.",
    type: "object",
    properties: {
      name: {
        description: "Project name.",
        type: "string",
        minLength: 1,
        maxLength: 128,
      },
      path: {
        description: "Save immediately at this absolute path.",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
      preset: {
        description: "Format preset id.",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
      frameRate: {
        description: "Frame rate.",
        type: [
          "number",
          "null",
        ],
        default: null,
        minimum: 1,
        maximum: 120,
      },
      cameraWidth: {
        description: "Camera width.",
        type: [
          "integer",
          "null",
        ],
        default: null,
        minimum: 16,
        maximum: 8192,
      },
      cameraHeight: {
        description: "Camera height.",
        type: [
          "integer",
          "null",
        ],
        default: null,
        minimum: 16,
        maximum: 8192,
      },
      onUnsaved: {
        description: "Unsaved-changes policy.",
        $ref: "#/$defs/project.create.OnUnsaved",
      },
      onExists: {
        description: "Overwrite policy.",
        $ref: "#/$defs/project.create.OnExists",
      },
    },
    additionalProperties: false,
    required: [
      "name",
    ],
  },
  "project.create.result": {
    title: "CreateResult",
    description: "`project.create` result.",
    type: "object",
    properties: {
      projectId: {
        description: "Project id.",
        type: "string",
      },
      path: {
        description: "Saved path.",
        type: [
          "string",
          "null",
        ],
      },
      sceneId: {
        description: "First scene.",
        type: "string",
      },
      cameraId: {
        description: "Its camera.",
        type: "string",
      },
      elementId: {
        description: "Its drawing element.",
        type: "string",
      },
    },
    required: [
      "projectId",
      "sceneId",
      "cameraId",
      "elementId",
    ],
  },
  "project.dirty.changed.params": {
    title: "NoParams",
    description: "No parameters.",
    type: "object",
    additionalProperties: false,
  },
  "project.dirty.changed.result": {
    title: "DirtyChanged",
    description: "`project.dirty.changed` payload.",
    type: "object",
    properties: {
      projectDirty: {
        description: "Project modified.",
        type: "boolean",
      },
      globalLibraryDirty: {
        description: "Global library modified.",
        type: "boolean",
      },
    },
    required: [
      "projectDirty",
      "globalLibraryDirty",
    ],
  },
  "project.info.get.params": {
    title: "NoParams",
    description: "No parameters.",
    type: "object",
    additionalProperties: false,
  },
  "project.info.get.result": {
    title: "Map_of_AnyValue",
    type: "object",
    additionalProperties: true,
  },
  "project.open.OnUnsaved": {
    description: "What to do with unsaved changes (01 §11.2 rule 3).",
    oneOf: [
      {
        description: "Fail with `project.unsavedChanges` when something is modified.",
        type: "string",
        const: "fail",
      },
      {
        description: "Save first.",
        type: "string",
        const: "save",
      },
      {
        description: "Discard the changes.",
        type: "string",
        const: "discard",
      },
    ],
  },
  "project.open.params": {
    title: "OpenParams",
    description: "`project.open` parameters.",
    type: "object",
    properties: {
      path: {
        description: "Absolute package path.",
        type: "string",
      },
      readOnly: {
        description: "Open read-only.",
        type: "boolean",
        default: false,
      },
      onUnsaved: {
        description: "Unsaved-changes policy.",
        $ref: "#/$defs/project.open.OnUnsaved",
      },
    },
    additionalProperties: false,
    required: [
      "path",
    ],
  },
  "project.open.result": {
    title: "JobStarted",
    description: "`{jobId}`.",
    type: "object",
    properties: {
      jobId: {
        description: "The job.",
        type: "string",
      },
    },
    required: [
      "jobId",
    ],
  },
  "project.opened.params": {
    title: "NoParams",
    description: "No parameters.",
    type: "object",
    additionalProperties: false,
  },
  "project.opened.result": {
    title: "Opened",
    description: "`project.opened` payload.",
    type: "object",
    properties: {
      projectId: {
        description: "Project id.",
        type: "string",
      },
      path: {
        description: "Path.",
        type: [
          "string",
          "null",
        ],
      },
      readOnly: {
        description: "Read-only.",
        type: "boolean",
      },
    },
    required: [
      "projectId",
      "readOnly",
    ],
  },
  "project.presets.list.params": {
    title: "NoParams",
    description: "No parameters.",
    type: "object",
    additionalProperties: false,
  },
  "project.presets.list.result": {
    title: "Array_of_Map_of_AnyValue",
    type: "array",
    items: {
      type: "object",
      additionalProperties: true,
    },
  },
  "project.properties.get.params": {
    title: "NoParams",
    description: "No parameters.",
    type: "object",
    additionalProperties: false,
  },
  "project.properties.get.result": {
    title: "Properties",
    description: "Project properties.",
    type: "object",
    properties: {
      frameRate: {
        description: "Frame rate.",
        type: "number",
      },
      cameraWidth: {
        description: "Camera width.",
        type: "integer",
        maximum: 2147483647,
        minimum: 0,
      },
      cameraHeight: {
        description: "Camera height.",
        type: "integer",
        maximum: 2147483647,
        minimum: 0,
      },
    },
    required: [
      "frameRate",
      "cameraWidth",
      "cameraHeight",
    ],
  },
  "project.properties.set.params": {
    title: "PropertiesSetParams",
    description: "`project.properties.set` parameters.",
    type: "object",
    properties: {
      frameRate: {
        description: "Frame rate.",
        type: [
          "number",
          "null",
        ],
        default: null,
        minimum: 1,
        maximum: 120,
      },
      cameraWidth: {
        description: "Camera width.",
        type: [
          "integer",
          "null",
        ],
        default: null,
        minimum: 16,
        maximum: 8192,
      },
      cameraHeight: {
        description: "Camera height.",
        type: [
          "integer",
          "null",
        ],
        default: null,
        minimum: 16,
        maximum: 8192,
      },
      setAsDefault: {
        description: "Make these the defaults for new projects.",
        type: "boolean",
        default: false,
      },
    },
    additionalProperties: false,
  },
  "project.properties.set.result": {
    title: "Properties",
    description: "Project properties.",
    type: "object",
    properties: {
      frameRate: {
        description: "Frame rate.",
        type: "number",
      },
      cameraWidth: {
        description: "Camera width.",
        type: "integer",
        maximum: 2147483647,
        minimum: 0,
      },
      cameraHeight: {
        description: "Camera height.",
        type: "integer",
        maximum: 2147483647,
        minimum: 0,
      },
    },
    required: [
      "frameRate",
      "cameraWidth",
      "cameraHeight",
    ],
  },
  "project.recent.list.params": {
    title: "NoParams",
    description: "No parameters.",
    type: "object",
    additionalProperties: false,
  },
  "project.recent.list.result": {
    title: "Array_of_Map_of_AnyValue",
    type: "array",
    items: {
      type: "object",
      additionalProperties: true,
    },
  },
  "project.recent.remove.params": {
    title: "RecentRemoveParams",
    description: "`project.recent.remove` parameters.",
    type: "object",
    properties: {
      path: {
        description: "One entry.",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
      all: {
        description: "Every entry.",
        type: "boolean",
        default: false,
      },
    },
    additionalProperties: false,
  },
  "project.recent.remove.result": {
    title: "Remaining",
    description: "`{remaining}`.",
    type: "object",
    properties: {
      remaining: {
        description: "Entries left.",
        type: "integer",
        maximum: 2147483647,
        minimum: 0,
      },
    },
    required: [
      "remaining",
    ],
  },
  "project.recovery.delete.params": {
    title: "RecoveryDeleteParams",
    description: "`project.recovery.delete` parameters.",
    type: "object",
    properties: {
      recoveryId: {
        description: "One entry.",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
      all: {
        description: "Every entry.",
        type: "boolean",
        default: false,
      },
    },
    additionalProperties: false,
  },
  "project.recovery.delete.result": {
    title: "Deleted",
    description: "`{deleted}`.",
    type: "object",
    properties: {
      deleted: {
        description: "Entries deleted.",
        type: "integer",
        maximum: 2147483647,
        minimum: 0,
      },
    },
    required: [
      "deleted",
    ],
  },
  "project.recovery.list.params": {
    title: "NoParams",
    description: "No parameters.",
    type: "object",
    additionalProperties: false,
  },
  "project.recovery.list.result": {
    title: "Array_of_Map_of_AnyValue",
    type: "array",
    items: {
      type: "object",
      additionalProperties: true,
    },
  },
  "project.recovery.open.OnUnsaved": {
    description: "What to do with unsaved changes (01 §11.2 rule 3).",
    oneOf: [
      {
        description: "Fail with `project.unsavedChanges` when something is modified.",
        type: "string",
        const: "fail",
      },
      {
        description: "Save first.",
        type: "string",
        const: "save",
      },
      {
        description: "Discard the changes.",
        type: "string",
        const: "discard",
      },
    ],
  },
  "project.recovery.open.params": {
    title: "RecoveryOpenParams",
    description: "`project.recovery.open` parameters.",
    type: "object",
    properties: {
      recoveryId: {
        description: "Recovery id.",
        type: "string",
      },
      onUnsaved: {
        description: "Unsaved-changes policy.",
        $ref: "#/$defs/project.recovery.open.OnUnsaved",
      },
    },
    additionalProperties: false,
    required: [
      "recoveryId",
    ],
  },
  "project.recovery.open.result": {
    title: "JobStarted",
    description: "`{jobId}`.",
    type: "object",
    properties: {
      jobId: {
        description: "The job.",
        type: "string",
      },
    },
    required: [
      "jobId",
    ],
  },
  "project.save.params": {
    title: "SaveParams",
    description: "`project.save` parameters.",
    type: "object",
    properties: {
      includeGlobalLibrary: {
        description: "Also save the global library.",
        type: "boolean",
        default: true,
      },
      allowFormatUpgrade: {
        description: "Allow writing a newer format version.",
        type: "boolean",
        default: false,
      },
    },
    additionalProperties: false,
  },
  "project.save.result": {
    title: "JobStarted",
    description: "`{jobId}`.",
    type: "object",
    properties: {
      jobId: {
        description: "The job.",
        type: "string",
      },
    },
    required: [
      "jobId",
    ],
  },
  "project.saveAs.OnExists": {
    description: "What to do when a target file exists (01 §11.2 rule 4).",
    oneOf: [
      {
        description: "Fail with `fs.exists` / `project.exists`.",
        type: "string",
        const: "fail",
      },
      {
        description: "Overwrite.",
        type: "string",
        const: "overwrite",
      },
    ],
  },
  "project.saveAs.params": {
    title: "SaveAsParams",
    description: "`project.saveAs` parameters.",
    type: "object",
    properties: {
      path: {
        description: "Absolute target path.",
        type: "string",
      },
      onExists: {
        description: "Overwrite policy.",
        $ref: "#/$defs/project.saveAs.OnExists",
      },
      includeGlobalLibrary: {
        description: "Also save the global library.",
        type: "boolean",
        default: true,
      },
      allowFormatUpgrade: {
        description: "Allow writing a newer format version.",
        type: "boolean",
        default: false,
      },
    },
    additionalProperties: false,
    required: [
      "path",
    ],
  },
  "project.saveAs.result": {
    title: "JobStarted",
    description: "`{jobId}`.",
    type: "object",
    properties: {
      jobId: {
        description: "The job.",
        type: "string",
      },
    },
    required: [
      "jobId",
    ],
  },
  "project.saved.params": {
    title: "NoParams",
    description: "No parameters.",
    type: "object",
    additionalProperties: false,
  },
  "project.saved.result": {
    title: "Map_of_AnyValue",
    type: "object",
    additionalProperties: true,
  },
  "window.floatingPanels.set.params": {
    title: "Visible",
    description: "`{visible}` parameters and result.",
    type: "object",
    properties: {
      visible: {
        description: "Show (true) or hide (false).",
        type: "boolean",
      },
    },
    additionalProperties: false,
    required: [
      "visible",
    ],
  },
  "window.floatingPanels.set.result": {
    title: "Visible",
    description: "`{visible}` parameters and result.",
    type: "object",
    properties: {
      visible: {
        description: "Show (true) or hide (false).",
        type: "boolean",
      },
    },
    additionalProperties: false,
    required: [
      "visible",
    ],
  },
  "window.fullScreen.set.params": {
    title: "Enabled",
    description: "`{enabled}` parameters and result.",
    type: "object",
    properties: {
      enabled: {
        description: "The state to set (explicit, never a toggle; readiness bullet 4).",
        type: "boolean",
      },
    },
    additionalProperties: false,
    required: [
      "enabled",
    ],
  },
  "window.fullScreen.set.result": {
    title: "Enabled",
    description: "`{enabled}` parameters and result.",
    type: "object",
    properties: {
      enabled: {
        description: "The state to set (explicit, never a toggle; readiness bullet 4).",
        type: "boolean",
      },
    },
    additionalProperties: false,
    required: [
      "enabled",
    ],
  },
  "window.geometry.get.params": {
    title: "NoParams",
    description: "No parameters.",
    type: "object",
    additionalProperties: false,
  },
  "window.geometry.get.Screen": {
    description: "A screen.",
    type: "object",
    properties: {
      x: {
        description: "Left.",
        type: "integer",
        maximum: 2147483647,
        minimum: -2147483648,
      },
      y: {
        description: "Top.",
        type: "integer",
        maximum: 2147483647,
        minimum: -2147483648,
      },
      width: {
        description: "Width.",
        type: "integer",
        maximum: 2147483647,
        minimum: 0,
      },
      height: {
        description: "Height.",
        type: "integer",
        maximum: 2147483647,
        minimum: 0,
      },
      scale: {
        description: "Scale factor.",
        type: "number",
      },
    },
    required: [
      "x",
      "y",
      "width",
      "height",
      "scale",
    ],
  },
  "window.geometry.get.result": {
    title: "WindowGeometry",
    description: "Window geometry.",
    type: "object",
    properties: {
      x: {
        description: "Left.",
        type: "integer",
        maximum: 2147483647,
        minimum: -2147483648,
      },
      y: {
        description: "Top.",
        type: "integer",
        maximum: 2147483647,
        minimum: -2147483648,
      },
      width: {
        description: "Width.",
        type: "integer",
        maximum: 2147483647,
        minimum: 0,
      },
      height: {
        description: "Height.",
        type: "integer",
        maximum: 2147483647,
        minimum: 0,
      },
      maximized: {
        description: "Maximised.",
        type: "boolean",
      },
      fullScreen: {
        description: "Full screen.",
        type: "boolean",
      },
      screens: {
        description: "Screens.",
        type: "array",
        items: {
          $ref: "#/$defs/window.geometry.get.Screen",
        },
      },
    },
    required: [
      "x",
      "y",
      "width",
      "height",
      "maximized",
      "fullScreen",
      "screens",
    ],
  },
  "window.geometry.set.params": {
    title: "GeometrySetParams",
    description: "`window.geometry.set` parameters.",
    type: "object",
    properties: {
      x: {
        description: "Left.",
        type: [
          "integer",
          "null",
        ],
        maximum: 2147483647,
        default: null,
        minimum: -2147483648,
      },
      y: {
        description: "Top.",
        type: [
          "integer",
          "null",
        ],
        maximum: 2147483647,
        default: null,
        minimum: -2147483648,
      },
      width: {
        description: "Width.",
        type: [
          "integer",
          "null",
        ],
        default: null,
        minimum: 1,
        maximum: 32767,
      },
      height: {
        description: "Height.",
        type: [
          "integer",
          "null",
        ],
        default: null,
        minimum: 1,
        maximum: 32767,
      },
      maximized: {
        description: "Maximised.",
        type: [
          "boolean",
          "null",
        ],
        default: null,
      },
    },
    additionalProperties: false,
  },
  "window.geometry.set.Screen": {
    description: "A screen.",
    type: "object",
    properties: {
      x: {
        description: "Left.",
        type: "integer",
        maximum: 2147483647,
        minimum: -2147483648,
      },
      y: {
        description: "Top.",
        type: "integer",
        maximum: 2147483647,
        minimum: -2147483648,
      },
      width: {
        description: "Width.",
        type: "integer",
        maximum: 2147483647,
        minimum: 0,
      },
      height: {
        description: "Height.",
        type: "integer",
        maximum: 2147483647,
        minimum: 0,
      },
      scale: {
        description: "Scale factor.",
        type: "number",
      },
    },
    required: [
      "x",
      "y",
      "width",
      "height",
      "scale",
    ],
  },
  "window.geometry.set.result": {
    title: "WindowGeometry",
    description: "Window geometry.",
    type: "object",
    properties: {
      x: {
        description: "Left.",
        type: "integer",
        maximum: 2147483647,
        minimum: -2147483648,
      },
      y: {
        description: "Top.",
        type: "integer",
        maximum: 2147483647,
        minimum: -2147483648,
      },
      width: {
        description: "Width.",
        type: "integer",
        maximum: 2147483647,
        minimum: 0,
      },
      height: {
        description: "Height.",
        type: "integer",
        maximum: 2147483647,
        minimum: 0,
      },
      maximized: {
        description: "Maximised.",
        type: "boolean",
      },
      fullScreen: {
        description: "Full screen.",
        type: "boolean",
      },
      screens: {
        description: "Screens.",
        type: "array",
        items: {
          $ref: "#/$defs/window.geometry.set.Screen",
        },
      },
    },
    required: [
      "x",
      "y",
      "width",
      "height",
      "maximized",
      "fullScreen",
      "screens",
    ],
  },
  "window.minimized.set.params": {
    title: "Minimized",
    description: "`{minimized}`.",
    type: "object",
    properties: {
      minimized: {
        description: "Minimised.",
        type: "boolean",
      },
    },
    additionalProperties: false,
    required: [
      "minimized",
    ],
  },
  "window.minimized.set.result": {
    title: "Minimized",
    description: "`{minimized}`.",
    type: "object",
    properties: {
      minimized: {
        description: "Minimised.",
        type: "boolean",
      },
    },
    additionalProperties: false,
    required: [
      "minimized",
    ],
  },
  "window.panel.list.params": {
    title: "NoParams",
    description: "No parameters.",
    type: "object",
    additionalProperties: false,
  },
  "window.panel.list.PanelEntry": {
    description: "A panel as listed by `window.panel.list` (01 A).",
    type: "object",
    properties: {
      panelId: {
        description: "Panel id.",
        type: "string",
      },
      title: {
        description: "Title.",
        type: "string",
      },
      visible: {
        description: "Shown.",
        type: "boolean",
      },
      floating: {
        description: "Floating.",
        type: "boolean",
      },
      area: {
        description: "Dock area (`left` … `bottom`, `central`).",
        type: [
          "string",
          "null",
        ],
      },
      tabGroup: {
        description: "Index of its tab group within the area.",
        type: [
          "integer",
          "null",
        ],
        format: "uint",
        minimum: 0,
      },
      current: {
        description: "In front of its tab group.",
        type: "boolean",
      },
      geometry: {
        description: "Floating geometry.",
        anyOf: [
          {
            $ref: "#/$defs/window.panel.list.Geometry",
          },
          {
            type: "null",
          },
        ],
      },
      focused: {
        description: "Has focus.",
        type: "boolean",
      },
    },
    required: [
      "panelId",
      "title",
      "visible",
      "floating",
      "current",
      "focused",
    ],
  },
  "window.panel.list.Geometry": {
    description: "A geometry in logical pixels.",
    type: "object",
    properties: {
      x: {
        description: "Left.",
        type: "integer",
        maximum: 2147483647,
        minimum: -2147483648,
      },
      y: {
        description: "Top.",
        type: "integer",
        maximum: 2147483647,
        minimum: -2147483648,
      },
      width: {
        description: "Width.",
        type: "integer",
        maximum: 32767,
        minimum: 1,
      },
      height: {
        description: "Height.",
        type: "integer",
        maximum: 32767,
        minimum: 1,
      },
    },
    additionalProperties: false,
    required: [
      "x",
      "y",
      "width",
      "height",
    ],
  },
  "window.panel.list.result": {
    title: "Array_of_PanelEntry",
    type: "array",
    items: {
      $ref: "#/$defs/window.panel.list.PanelEntry",
    },
  },
  "window.panel.move.Area": {
    description: "Dock area.",
    oneOf: [
      {
        description: "Left.",
        type: "string",
        const: "left",
      },
      {
        description: "Right.",
        type: "string",
        const: "right",
      },
      {
        description: "Top.",
        type: "string",
        const: "top",
      },
      {
        description: "Bottom.",
        type: "string",
        const: "bottom",
      },
      {
        description: "The central view.",
        type: "string",
        const: "central",
      },
    ],
  },
  "window.panel.move.Placement": {
    description: "Placement relative to `relativeTo`.",
    oneOf: [
      {
        description: "As a tab.",
        type: "string",
        const: "tab",
      },
      {
        description: "Before.",
        type: "string",
        const: "before",
      },
      {
        description: "After.",
        type: "string",
        const: "after",
      },
      {
        description: "Above.",
        type: "string",
        const: "above",
      },
      {
        description: "Below.",
        type: "string",
        const: "below",
      },
    ],
  },
  "window.panel.move.params": {
    title: "PanelMoveParams",
    description: "`window.panel.move` parameters.",
    type: "object",
    properties: {
      panelId: {
        description: "Panel id.",
        type: "string",
      },
      area: {
        description: "Target area.",
        $ref: "#/$defs/window.panel.move.Area",
      },
      relativeTo: {
        description: "A panel docked in the area.",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
      position: {
        description: "Placement.",
        $ref: "#/$defs/window.panel.move.Placement",
      },
      size: {
        description: "Area width or height.",
        type: [
          "integer",
          "null",
        ],
        default: null,
        minimum: 1,
        maximum: 32767,
      },
    },
    additionalProperties: false,
    required: [
      "panelId",
      "area",
    ],
  },
  "window.panel.move.Corners": {
    description: "Corner ownership (01 §5.4 *Extend side panels*).",
    oneOf: [
      {
        description: "Left and right areas run full height.",
        type: "string",
        const: "sides",
      },
      {
        description: "Top and bottom areas run full width.",
        type: "string",
        const: "topBottom",
      },
    ],
  },
  "window.panel.move.Docks": {
    description: "The four dock areas, each an ordered stack of tab groups.",
    type: "object",
    properties: {
      left: {
        description: "Left area.",
        type: "array",
        items: {
          $ref: "#/$defs/window.panel.move.TabGroup",
        },
        default: [],
      },
      right: {
        description: "Right area.",
        type: "array",
        items: {
          $ref: "#/$defs/window.panel.move.TabGroup",
        },
        default: [],
      },
      top: {
        description: "Top area.",
        type: "array",
        items: {
          $ref: "#/$defs/window.panel.move.TabGroup",
        },
        default: [],
      },
      bottom: {
        description: "Bottom area.",
        type: "array",
        items: {
          $ref: "#/$defs/window.panel.move.TabGroup",
        },
        default: [],
      },
    },
    additionalProperties: false,
  },
  "window.panel.move.TabGroup": {
    description: "One tab group of a dock area.",
    type: "object",
    properties: {
      tabs: {
        description: "Panel ids, in tab order.",
        type: "array",
        items: {
          type: "string",
        },
      },
      current: {
        description: "The tab in front.",
        type: "string",
      },
      size: {
        description: "On the first group of an area: the area's width (left/right) or height (top/bottom).",
        type: [
          "integer",
          "null",
        ],
        maximum: 2147483647,
        minimum: 0,
      },
    },
    additionalProperties: false,
    required: [
      "tabs",
      "current",
    ],
  },
  "window.panel.move.Floating": {
    description: "A floating panel.",
    type: "object",
    properties: {
      panelId: {
        description: "Panel id.",
        type: "string",
      },
      visible: {
        description: "Shown.",
        type: "boolean",
      },
      geometry: {
        description: "Window geometry.",
        $ref: "#/$defs/window.panel.move.Geometry",
      },
    },
    additionalProperties: false,
    required: [
      "panelId",
      "visible",
      "geometry",
    ],
  },
  "window.panel.move.Geometry": {
    description: "A geometry in logical pixels.",
    type: "object",
    properties: {
      x: {
        description: "Left.",
        type: "integer",
        maximum: 2147483647,
        minimum: -2147483648,
      },
      y: {
        description: "Top.",
        type: "integer",
        maximum: 2147483647,
        minimum: -2147483648,
      },
      width: {
        description: "Width.",
        type: "integer",
        maximum: 32767,
        minimum: 1,
      },
      height: {
        description: "Height.",
        type: "integer",
        maximum: 32767,
        minimum: 1,
      },
    },
    additionalProperties: false,
    required: [
      "x",
      "y",
      "width",
      "height",
    ],
  },
  "window.panel.move.ToolPalette": {
    description: "A tool palette.",
    type: "object",
    properties: {
      id: {
        description: "Palette id (`drawingTools`, …).",
        type: "string",
      },
      visible: {
        description: "Shown.",
        type: "boolean",
      },
      floating: {
        description: "Floating (else docked with the main toolbar).",
        type: "boolean",
      },
      position: {
        description: "Floating position.",
        anyOf: [
          {
            $ref: "#/$defs/window.panel.move.Position",
          },
          {
            type: "null",
          },
        ],
      },
    },
    additionalProperties: false,
    required: [
      "id",
      "visible",
      "floating",
    ],
  },
  "window.panel.move.Position": {
    description: "A point in logical pixels.",
    type: "object",
    properties: {
      x: {
        description: "Left.",
        type: "integer",
        maximum: 2147483647,
        minimum: -2147483648,
      },
      y: {
        description: "Top.",
        type: "integer",
        maximum: 2147483647,
        minimum: -2147483648,
      },
    },
    additionalProperties: false,
    required: [
      "x",
      "y",
    ],
  },
  "window.panel.move.MainToolbar": {
    description: "The main toolbar configuration (01 §5.7).",
    type: "object",
    properties: {
      visible: {
        description: "Shown.",
        type: "boolean",
      },
      items: {
        description: "Items: command references, group ids, `separator`, `space`, `flexibleSpace`.",
        type: "array",
        items: {
          type: "string",
        },
      },
    },
    additionalProperties: false,
    required: [
      "visible",
      "items",
    ],
  },
  "window.panel.move.Arrangement": {
    description: "An arrangement of the main window (01 §12.4 `Layout`, without the nested full-screen arrangement).",
    type: "object",
    properties: {
      centralPanel: {
        description: "The central view.",
        type: "string",
      },
      corners: {
        description: "Corner ownership.",
        $ref: "#/$defs/window.panel.move.Corners",
      },
      docks: {
        description: "Dock areas.",
        $ref: "#/$defs/window.panel.move.Docks",
      },
      floating: {
        description: "Floating panels.",
        type: "array",
        items: {
          $ref: "#/$defs/window.panel.move.Floating",
        },
        default: [],
      },
      toolPalettes: {
        description: "Tool palettes.",
        type: "array",
        items: {
          $ref: "#/$defs/window.panel.move.ToolPalette",
        },
        default: [],
      },
      mainToolbar: {
        description: "Main toolbar.",
        $ref: "#/$defs/window.panel.move.MainToolbar",
      },
    },
    additionalProperties: false,
    required: [
      "centralPanel",
      "corners",
      "docks",
      "mainToolbar",
    ],
  },
  "window.panel.move.result": {
    title: "Layout",
    description: "A workspace layout on the wire (01 §12.4 `Layout`): the normal arrangement's fields plus the separate\nfull-screen arrangement (01 §5.6). Panels not mentioned are hidden.",
    type: "object",
    properties: {
      centralPanel: {
        description: "The central view.",
        type: "string",
      },
      corners: {
        description: "Corner ownership.",
        $ref: "#/$defs/window.panel.move.Corners",
      },
      docks: {
        description: "Dock areas.",
        $ref: "#/$defs/window.panel.move.Docks",
      },
      floating: {
        description: "Floating panels.",
        type: "array",
        items: {
          $ref: "#/$defs/window.panel.move.Floating",
        },
        default: [],
      },
      toolPalettes: {
        description: "Tool palettes.",
        type: "array",
        items: {
          $ref: "#/$defs/window.panel.move.ToolPalette",
        },
        default: [],
      },
      mainToolbar: {
        description: "Main toolbar.",
        $ref: "#/$defs/window.panel.move.MainToolbar",
      },
      fullScreen: {
        description: "Full-screen arrangement (same shape without `fullScreen`).",
        $ref: "#/$defs/window.panel.move.Arrangement",
      },
    },
    additionalProperties: false,
    required: [
      "centralPanel",
      "corners",
      "docks",
      "mainToolbar",
      "fullScreen",
    ],
  },
  "window.panel.set.Geometry": {
    description: "A geometry in logical pixels.",
    type: "object",
    properties: {
      x: {
        description: "Left.",
        type: "integer",
        maximum: 2147483647,
        minimum: -2147483648,
      },
      y: {
        description: "Top.",
        type: "integer",
        maximum: 2147483647,
        minimum: -2147483648,
      },
      width: {
        description: "Width.",
        type: "integer",
        maximum: 32767,
        minimum: 1,
      },
      height: {
        description: "Height.",
        type: "integer",
        maximum: 32767,
        minimum: 1,
      },
    },
    additionalProperties: false,
    required: [
      "x",
      "y",
      "width",
      "height",
    ],
  },
  "window.panel.set.params": {
    title: "PanelSetParams",
    description: "`window.panel.set` parameters.",
    type: "object",
    properties: {
      panelId: {
        description: "Panel id (01 §5.2) or `active` for the focused panel.",
        type: "string",
      },
      visible: {
        description: "Show or hide.",
        type: [
          "boolean",
          "null",
        ],
        default: null,
      },
      floating: {
        description: "Float or dock.",
        type: [
          "boolean",
          "null",
        ],
        default: null,
      },
      focus: {
        description: "Give it focus.",
        type: "boolean",
        default: true,
      },
      geometry: {
        description: "Floating geometry.",
        anyOf: [
          {
            $ref: "#/$defs/window.panel.set.Geometry",
          },
          {
            type: "null",
          },
        ],
        default: null,
      },
    },
    additionalProperties: false,
    required: [
      "panelId",
    ],
  },
  "window.panel.set.result": {
    title: "PanelEntry",
    description: "A panel as listed by `window.panel.list` (01 A).",
    type: "object",
    properties: {
      panelId: {
        description: "Panel id.",
        type: "string",
      },
      title: {
        description: "Title.",
        type: "string",
      },
      visible: {
        description: "Shown.",
        type: "boolean",
      },
      floating: {
        description: "Floating.",
        type: "boolean",
      },
      area: {
        description: "Dock area (`left` … `bottom`, `central`).",
        type: [
          "string",
          "null",
        ],
      },
      tabGroup: {
        description: "Index of its tab group within the area.",
        type: [
          "integer",
          "null",
        ],
        format: "uint",
        minimum: 0,
      },
      current: {
        description: "In front of its tab group.",
        type: "boolean",
      },
      geometry: {
        description: "Floating geometry.",
        anyOf: [
          {
            $ref: "#/$defs/window.panel.set.Geometry",
          },
          {
            type: "null",
          },
        ],
      },
      focused: {
        description: "Has focus.",
        type: "boolean",
      },
    },
    required: [
      "panelId",
      "title",
      "visible",
      "floating",
      "current",
      "focused",
    ],
  },
  "window.raiseAll.run.params": {
    title: "NoParams",
    description: "No parameters.",
    type: "object",
    additionalProperties: false,
  },
  "window.raiseAll.run.result": {
    title: "Empty",
    description: "An empty result object.",
    type: "object",
  },
  "window.sidePanelsExtended.set.params": {
    title: "Enabled",
    description: "`{enabled}` parameters and result.",
    type: "object",
    properties: {
      enabled: {
        description: "The state to set (explicit, never a toggle; readiness bullet 4).",
        type: "boolean",
      },
    },
    additionalProperties: false,
    required: [
      "enabled",
    ],
  },
  "window.sidePanelsExtended.set.result": {
    title: "Enabled",
    description: "`{enabled}` parameters and result.",
    type: "object",
    properties: {
      enabled: {
        description: "The state to set (explicit, never a toggle; readiness bullet 4).",
        type: "boolean",
      },
    },
    additionalProperties: false,
    required: [
      "enabled",
    ],
  },
  "window.toolbar.items.reset.MainOnly": {
    description: "Only the main toolbar is configurable.",
    oneOf: [
      {
        description: "The main toolbar.",
        type: "string",
        const: "main",
      },
    ],
  },
  "window.toolbar.items.reset.params": {
    title: "ToolbarResetParams",
    description: "`window.toolbar.items.reset` parameters.",
    type: "object",
    properties: {
      toolbarId: {
        description: "The main toolbar.",
        $ref: "#/$defs/window.toolbar.items.reset.MainOnly",
      },
    },
    additionalProperties: false,
  },
  "window.toolbar.items.reset.result": {
    title: "ItemsResult",
    description: "`{items}`.",
    type: "object",
    properties: {
      items: {
        description: "The toolbar's items.",
        type: "array",
        items: {
          type: "string",
        },
      },
    },
    required: [
      "items",
    ],
  },
  "window.toolbar.items.set.MainOnly": {
    description: "Only the main toolbar is configurable.",
    oneOf: [
      {
        description: "The main toolbar.",
        type: "string",
        const: "main",
      },
    ],
  },
  "window.toolbar.items.set.params": {
    title: "ToolbarItemsParams",
    description: "`window.toolbar.items.set` parameters.",
    type: "object",
    properties: {
      toolbarId: {
        description: "The main toolbar.",
        $ref: "#/$defs/window.toolbar.items.set.MainOnly",
      },
      items: {
        description: "Items: command references, group ids, `separator`, `space`, `flexibleSpace`.",
        type: "array",
        items: {
          type: "string",
        },
        maxItems: 200,
      },
    },
    additionalProperties: false,
    required: [
      "items",
    ],
  },
  "window.toolbar.items.set.result": {
    title: "ItemsResult",
    description: "`{items}`.",
    type: "object",
    properties: {
      items: {
        description: "The toolbar's items.",
        type: "array",
        items: {
          type: "string",
        },
      },
    },
    required: [
      "items",
    ],
  },
  "window.toolbar.list.params": {
    title: "NoParams",
    description: "No parameters.",
    type: "object",
    additionalProperties: false,
  },
  "window.toolbar.list.ToolbarEntry": {
    description: "A toolbar as listed (01 §5.7).",
    type: "object",
    properties: {
      toolbarId: {
        description: "`main` or a tool palette id.",
        type: "string",
      },
      visible: {
        description: "Shown.",
        type: "boolean",
      },
      floating: {
        description: "Floating.",
        type: "boolean",
      },
      position: {
        description: "Floating position.",
        anyOf: [
          {
            $ref: "#/$defs/window.toolbar.list.Position",
          },
          {
            type: "null",
          },
        ],
      },
      items: {
        description: "Items (main toolbar).",
        type: "array",
        items: {
          type: "string",
        },
      },
      allowedItems: {
        description: "Items that may be placed (main toolbar).",
        type: "array",
        items: {
          type: "string",
        },
      },
    },
    required: [
      "toolbarId",
      "visible",
      "floating",
      "items",
      "allowedItems",
    ],
  },
  "window.toolbar.list.Position": {
    description: "A point in logical pixels.",
    type: "object",
    properties: {
      x: {
        description: "Left.",
        type: "integer",
        maximum: 2147483647,
        minimum: -2147483648,
      },
      y: {
        description: "Top.",
        type: "integer",
        maximum: 2147483647,
        minimum: -2147483648,
      },
    },
    additionalProperties: false,
    required: [
      "x",
      "y",
    ],
  },
  "window.toolbar.list.result": {
    title: "Array_of_ToolbarEntry",
    type: "array",
    items: {
      $ref: "#/$defs/window.toolbar.list.ToolbarEntry",
    },
  },
  "window.toolbar.set.ToolbarId": {
    description: "Toolbar ids.",
    oneOf: [
      {
        description: "The main toolbar.",
        type: "string",
        const: "main",
      },
      {
        description: "Drawing tools palette.",
        type: "string",
        const: "drawingTools",
      },
      {
        description: "Animation tools palette.",
        type: "string",
        const: "animationTools",
      },
      {
        description: "Bone tools palette.",
        type: "string",
        const: "boneTools",
      },
      {
        description: "Effect tools palette.",
        type: "string",
        const: "effectTools",
      },
      {
        description: "Sound tools palette.",
        type: "string",
        const: "soundTools",
      },
    ],
  },
  "window.toolbar.set.Position": {
    description: "A point in logical pixels.",
    type: "object",
    properties: {
      x: {
        description: "Left.",
        type: "integer",
        maximum: 2147483647,
        minimum: -2147483648,
      },
      y: {
        description: "Top.",
        type: "integer",
        maximum: 2147483647,
        minimum: -2147483648,
      },
    },
    additionalProperties: false,
    required: [
      "x",
      "y",
    ],
  },
  "window.toolbar.set.params": {
    title: "ToolbarSetParams",
    description: "`window.toolbar.set` parameters.",
    type: "object",
    properties: {
      toolbarId: {
        description: "The toolbar.",
        $ref: "#/$defs/window.toolbar.set.ToolbarId",
      },
      visible: {
        description: "Shown.",
        type: "boolean",
      },
      floating: {
        description: "Floating (tool palettes).",
        type: [
          "boolean",
          "null",
        ],
        default: null,
      },
      position: {
        description: "Floating position.",
        anyOf: [
          {
            $ref: "#/$defs/window.toolbar.set.Position",
          },
          {
            type: "null",
          },
        ],
        default: null,
      },
    },
    additionalProperties: false,
    required: [
      "toolbarId",
      "visible",
    ],
  },
  "window.toolbar.set.result": {
    title: "ToolbarEntry",
    description: "A toolbar as listed (01 §5.7).",
    type: "object",
    properties: {
      toolbarId: {
        description: "`main` or a tool palette id.",
        type: "string",
      },
      visible: {
        description: "Shown.",
        type: "boolean",
      },
      floating: {
        description: "Floating.",
        type: "boolean",
      },
      position: {
        description: "Floating position.",
        anyOf: [
          {
            $ref: "#/$defs/window.toolbar.set.Position",
          },
          {
            type: "null",
          },
        ],
      },
      items: {
        description: "Items (main toolbar).",
        type: "array",
        items: {
          type: "string",
        },
      },
      allowedItems: {
        description: "Items that may be placed (main toolbar).",
        type: "array",
        items: {
          type: "string",
        },
      },
    },
    required: [
      "toolbarId",
      "visible",
      "floating",
      "items",
      "allowedItems",
    ],
  },
  "workspace.changed.params": {
    title: "NoParams",
    description: "No parameters.",
    type: "object",
    additionalProperties: false,
  },
  "workspace.changed.result": {
    title: "Changed",
    description: "`workspace.changed` payload.",
    type: "object",
    properties: {
      name: {
        description: "Current workspace.",
        type: "string",
      },
      modified: {
        description: "The arrangement differs from the stored one.",
        type: "boolean",
      },
    },
    required: [
      "name",
      "modified",
    ],
  },
  "workspace.create.Layout": {
    description: "A workspace layout on the wire (01 §12.4 `Layout`): the normal arrangement's fields plus the separate\nfull-screen arrangement (01 §5.6). Panels not mentioned are hidden.",
    type: "object",
    properties: {
      centralPanel: {
        description: "The central view.",
        type: "string",
      },
      corners: {
        description: "Corner ownership.",
        $ref: "#/$defs/workspace.create.Corners",
      },
      docks: {
        description: "Dock areas.",
        $ref: "#/$defs/workspace.create.Docks",
      },
      floating: {
        description: "Floating panels.",
        type: "array",
        items: {
          $ref: "#/$defs/workspace.create.Floating",
        },
        default: [],
      },
      toolPalettes: {
        description: "Tool palettes.",
        type: "array",
        items: {
          $ref: "#/$defs/workspace.create.ToolPalette",
        },
        default: [],
      },
      mainToolbar: {
        description: "Main toolbar.",
        $ref: "#/$defs/workspace.create.MainToolbar",
      },
      fullScreen: {
        description: "Full-screen arrangement (same shape without `fullScreen`).",
        $ref: "#/$defs/workspace.create.Arrangement",
      },
    },
    additionalProperties: false,
    required: [
      "centralPanel",
      "corners",
      "docks",
      "mainToolbar",
      "fullScreen",
    ],
  },
  "workspace.create.Corners": {
    description: "Corner ownership (01 §5.4 *Extend side panels*).",
    oneOf: [
      {
        description: "Left and right areas run full height.",
        type: "string",
        const: "sides",
      },
      {
        description: "Top and bottom areas run full width.",
        type: "string",
        const: "topBottom",
      },
    ],
  },
  "workspace.create.Docks": {
    description: "The four dock areas, each an ordered stack of tab groups.",
    type: "object",
    properties: {
      left: {
        description: "Left area.",
        type: "array",
        items: {
          $ref: "#/$defs/workspace.create.TabGroup",
        },
        default: [],
      },
      right: {
        description: "Right area.",
        type: "array",
        items: {
          $ref: "#/$defs/workspace.create.TabGroup",
        },
        default: [],
      },
      top: {
        description: "Top area.",
        type: "array",
        items: {
          $ref: "#/$defs/workspace.create.TabGroup",
        },
        default: [],
      },
      bottom: {
        description: "Bottom area.",
        type: "array",
        items: {
          $ref: "#/$defs/workspace.create.TabGroup",
        },
        default: [],
      },
    },
    additionalProperties: false,
  },
  "workspace.create.TabGroup": {
    description: "One tab group of a dock area.",
    type: "object",
    properties: {
      tabs: {
        description: "Panel ids, in tab order.",
        type: "array",
        items: {
          type: "string",
        },
      },
      current: {
        description: "The tab in front.",
        type: "string",
      },
      size: {
        description: "On the first group of an area: the area's width (left/right) or height (top/bottom).",
        type: [
          "integer",
          "null",
        ],
        maximum: 2147483647,
        minimum: 0,
      },
    },
    additionalProperties: false,
    required: [
      "tabs",
      "current",
    ],
  },
  "workspace.create.Floating": {
    description: "A floating panel.",
    type: "object",
    properties: {
      panelId: {
        description: "Panel id.",
        type: "string",
      },
      visible: {
        description: "Shown.",
        type: "boolean",
      },
      geometry: {
        description: "Window geometry.",
        $ref: "#/$defs/workspace.create.Geometry",
      },
    },
    additionalProperties: false,
    required: [
      "panelId",
      "visible",
      "geometry",
    ],
  },
  "workspace.create.Geometry": {
    description: "A geometry in logical pixels.",
    type: "object",
    properties: {
      x: {
        description: "Left.",
        type: "integer",
        maximum: 2147483647,
        minimum: -2147483648,
      },
      y: {
        description: "Top.",
        type: "integer",
        maximum: 2147483647,
        minimum: -2147483648,
      },
      width: {
        description: "Width.",
        type: "integer",
        maximum: 32767,
        minimum: 1,
      },
      height: {
        description: "Height.",
        type: "integer",
        maximum: 32767,
        minimum: 1,
      },
    },
    additionalProperties: false,
    required: [
      "x",
      "y",
      "width",
      "height",
    ],
  },
  "workspace.create.ToolPalette": {
    description: "A tool palette.",
    type: "object",
    properties: {
      id: {
        description: "Palette id (`drawingTools`, …).",
        type: "string",
      },
      visible: {
        description: "Shown.",
        type: "boolean",
      },
      floating: {
        description: "Floating (else docked with the main toolbar).",
        type: "boolean",
      },
      position: {
        description: "Floating position.",
        anyOf: [
          {
            $ref: "#/$defs/workspace.create.Position",
          },
          {
            type: "null",
          },
        ],
      },
    },
    additionalProperties: false,
    required: [
      "id",
      "visible",
      "floating",
    ],
  },
  "workspace.create.Position": {
    description: "A point in logical pixels.",
    type: "object",
    properties: {
      x: {
        description: "Left.",
        type: "integer",
        maximum: 2147483647,
        minimum: -2147483648,
      },
      y: {
        description: "Top.",
        type: "integer",
        maximum: 2147483647,
        minimum: -2147483648,
      },
    },
    additionalProperties: false,
    required: [
      "x",
      "y",
    ],
  },
  "workspace.create.MainToolbar": {
    description: "The main toolbar configuration (01 §5.7).",
    type: "object",
    properties: {
      visible: {
        description: "Shown.",
        type: "boolean",
      },
      items: {
        description: "Items: command references, group ids, `separator`, `space`, `flexibleSpace`.",
        type: "array",
        items: {
          type: "string",
        },
      },
    },
    additionalProperties: false,
    required: [
      "visible",
      "items",
    ],
  },
  "workspace.create.Arrangement": {
    description: "An arrangement of the main window (01 §12.4 `Layout`, without the nested full-screen arrangement).",
    type: "object",
    properties: {
      centralPanel: {
        description: "The central view.",
        type: "string",
      },
      corners: {
        description: "Corner ownership.",
        $ref: "#/$defs/workspace.create.Corners",
      },
      docks: {
        description: "Dock areas.",
        $ref: "#/$defs/workspace.create.Docks",
      },
      floating: {
        description: "Floating panels.",
        type: "array",
        items: {
          $ref: "#/$defs/workspace.create.Floating",
        },
        default: [],
      },
      toolPalettes: {
        description: "Tool palettes.",
        type: "array",
        items: {
          $ref: "#/$defs/workspace.create.ToolPalette",
        },
        default: [],
      },
      mainToolbar: {
        description: "Main toolbar.",
        $ref: "#/$defs/workspace.create.MainToolbar",
      },
    },
    additionalProperties: false,
    required: [
      "centralPanel",
      "corners",
      "docks",
      "mainToolbar",
    ],
  },
  "workspace.create.params": {
    title: "CreateParams",
    description: "`workspace.create` parameters.",
    type: "object",
    properties: {
      name: {
        description: "New workspace name.",
        type: "string",
      },
      position: {
        description: "1-based position in the list (default: last).",
        type: [
          "integer",
          "null",
        ],
        format: "uint",
        minimum: 1,
        maximum: 1000,
        default: null,
      },
      layout: {
        description: "Layout (default: the current arrangement).",
        anyOf: [
          {
            $ref: "#/$defs/workspace.create.Layout",
          },
          {
            type: "null",
          },
        ],
        default: null,
      },
    },
    additionalProperties: false,
    required: [
      "name",
    ],
  },
  "workspace.create.result": {
    title: "CreateResult",
    description: "`{name, position}`.",
    type: "object",
    properties: {
      name: {
        description: "Name.",
        type: "string",
      },
      position: {
        description: "Position.",
        type: "integer",
        format: "uint",
        minimum: 0,
      },
    },
    required: [
      "name",
      "position",
    ],
  },
  "workspace.delete.params": {
    title: "NameParams",
    description: "`{name}` parameters.",
    type: "object",
    properties: {
      name: {
        description: "Workspace name.",
        type: "string",
      },
    },
    additionalProperties: false,
    required: [
      "name",
    ],
  },
  "workspace.delete.result": {
    title: "DeleteResult",
    description: "`{deleted}`.",
    type: "object",
    properties: {
      deleted: {
        description: "Deleted workspace.",
        type: "string",
      },
    },
    required: [
      "deleted",
    ],
  },
  "workspace.export.OnExists": {
    description: "What to do when a target file exists (01 §11.2 rule 4).",
    oneOf: [
      {
        description: "Fail with `fs.exists` / `project.exists`.",
        type: "string",
        const: "fail",
      },
      {
        description: "Overwrite.",
        type: "string",
        const: "overwrite",
      },
    ],
  },
  "workspace.export.params": {
    title: "ExportParams",
    description: "`workspace.export` parameters.",
    type: "object",
    properties: {
      name: {
        description: "Workspace name.",
        type: "string",
      },
      path: {
        description: "Absolute target path.",
        type: "string",
      },
      onExists: {
        description: "Overwrite policy.",
        $ref: "#/$defs/workspace.export.OnExists",
      },
    },
    additionalProperties: false,
    required: [
      "name",
      "path",
    ],
  },
  "workspace.export.result": {
    title: "PathResult",
    description: "`{path}`.",
    type: "object",
    properties: {
      path: {
        description: "The file written.",
        type: "string",
      },
    },
    required: [
      "path",
    ],
  },
  "workspace.get.params": {
    title: "OptionalName",
    description: "`{name?}` parameters.",
    type: "object",
    properties: {
      name: {
        description: "Workspace name (default: the current arrangement).",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
    },
    additionalProperties: false,
  },
  "workspace.get.Corners": {
    description: "Corner ownership (01 §5.4 *Extend side panels*).",
    oneOf: [
      {
        description: "Left and right areas run full height.",
        type: "string",
        const: "sides",
      },
      {
        description: "Top and bottom areas run full width.",
        type: "string",
        const: "topBottom",
      },
    ],
  },
  "workspace.get.Docks": {
    description: "The four dock areas, each an ordered stack of tab groups.",
    type: "object",
    properties: {
      left: {
        description: "Left area.",
        type: "array",
        items: {
          $ref: "#/$defs/workspace.get.TabGroup",
        },
        default: [],
      },
      right: {
        description: "Right area.",
        type: "array",
        items: {
          $ref: "#/$defs/workspace.get.TabGroup",
        },
        default: [],
      },
      top: {
        description: "Top area.",
        type: "array",
        items: {
          $ref: "#/$defs/workspace.get.TabGroup",
        },
        default: [],
      },
      bottom: {
        description: "Bottom area.",
        type: "array",
        items: {
          $ref: "#/$defs/workspace.get.TabGroup",
        },
        default: [],
      },
    },
    additionalProperties: false,
  },
  "workspace.get.TabGroup": {
    description: "One tab group of a dock area.",
    type: "object",
    properties: {
      tabs: {
        description: "Panel ids, in tab order.",
        type: "array",
        items: {
          type: "string",
        },
      },
      current: {
        description: "The tab in front.",
        type: "string",
      },
      size: {
        description: "On the first group of an area: the area's width (left/right) or height (top/bottom).",
        type: [
          "integer",
          "null",
        ],
        maximum: 2147483647,
        minimum: 0,
      },
    },
    additionalProperties: false,
    required: [
      "tabs",
      "current",
    ],
  },
  "workspace.get.Floating": {
    description: "A floating panel.",
    type: "object",
    properties: {
      panelId: {
        description: "Panel id.",
        type: "string",
      },
      visible: {
        description: "Shown.",
        type: "boolean",
      },
      geometry: {
        description: "Window geometry.",
        $ref: "#/$defs/workspace.get.Geometry",
      },
    },
    additionalProperties: false,
    required: [
      "panelId",
      "visible",
      "geometry",
    ],
  },
  "workspace.get.Geometry": {
    description: "A geometry in logical pixels.",
    type: "object",
    properties: {
      x: {
        description: "Left.",
        type: "integer",
        maximum: 2147483647,
        minimum: -2147483648,
      },
      y: {
        description: "Top.",
        type: "integer",
        maximum: 2147483647,
        minimum: -2147483648,
      },
      width: {
        description: "Width.",
        type: "integer",
        maximum: 32767,
        minimum: 1,
      },
      height: {
        description: "Height.",
        type: "integer",
        maximum: 32767,
        minimum: 1,
      },
    },
    additionalProperties: false,
    required: [
      "x",
      "y",
      "width",
      "height",
    ],
  },
  "workspace.get.ToolPalette": {
    description: "A tool palette.",
    type: "object",
    properties: {
      id: {
        description: "Palette id (`drawingTools`, …).",
        type: "string",
      },
      visible: {
        description: "Shown.",
        type: "boolean",
      },
      floating: {
        description: "Floating (else docked with the main toolbar).",
        type: "boolean",
      },
      position: {
        description: "Floating position.",
        anyOf: [
          {
            $ref: "#/$defs/workspace.get.Position",
          },
          {
            type: "null",
          },
        ],
      },
    },
    additionalProperties: false,
    required: [
      "id",
      "visible",
      "floating",
    ],
  },
  "workspace.get.Position": {
    description: "A point in logical pixels.",
    type: "object",
    properties: {
      x: {
        description: "Left.",
        type: "integer",
        maximum: 2147483647,
        minimum: -2147483648,
      },
      y: {
        description: "Top.",
        type: "integer",
        maximum: 2147483647,
        minimum: -2147483648,
      },
    },
    additionalProperties: false,
    required: [
      "x",
      "y",
    ],
  },
  "workspace.get.MainToolbar": {
    description: "The main toolbar configuration (01 §5.7).",
    type: "object",
    properties: {
      visible: {
        description: "Shown.",
        type: "boolean",
      },
      items: {
        description: "Items: command references, group ids, `separator`, `space`, `flexibleSpace`.",
        type: "array",
        items: {
          type: "string",
        },
      },
    },
    additionalProperties: false,
    required: [
      "visible",
      "items",
    ],
  },
  "workspace.get.Arrangement": {
    description: "An arrangement of the main window (01 §12.4 `Layout`, without the nested full-screen arrangement).",
    type: "object",
    properties: {
      centralPanel: {
        description: "The central view.",
        type: "string",
      },
      corners: {
        description: "Corner ownership.",
        $ref: "#/$defs/workspace.get.Corners",
      },
      docks: {
        description: "Dock areas.",
        $ref: "#/$defs/workspace.get.Docks",
      },
      floating: {
        description: "Floating panels.",
        type: "array",
        items: {
          $ref: "#/$defs/workspace.get.Floating",
        },
        default: [],
      },
      toolPalettes: {
        description: "Tool palettes.",
        type: "array",
        items: {
          $ref: "#/$defs/workspace.get.ToolPalette",
        },
        default: [],
      },
      mainToolbar: {
        description: "Main toolbar.",
        $ref: "#/$defs/workspace.get.MainToolbar",
      },
    },
    additionalProperties: false,
    required: [
      "centralPanel",
      "corners",
      "docks",
      "mainToolbar",
    ],
  },
  "workspace.get.result": {
    title: "Layout",
    description: "A workspace layout on the wire (01 §12.4 `Layout`): the normal arrangement's fields plus the separate\nfull-screen arrangement (01 §5.6). Panels not mentioned are hidden.",
    type: "object",
    properties: {
      centralPanel: {
        description: "The central view.",
        type: "string",
      },
      corners: {
        description: "Corner ownership.",
        $ref: "#/$defs/workspace.get.Corners",
      },
      docks: {
        description: "Dock areas.",
        $ref: "#/$defs/workspace.get.Docks",
      },
      floating: {
        description: "Floating panels.",
        type: "array",
        items: {
          $ref: "#/$defs/workspace.get.Floating",
        },
        default: [],
      },
      toolPalettes: {
        description: "Tool palettes.",
        type: "array",
        items: {
          $ref: "#/$defs/workspace.get.ToolPalette",
        },
        default: [],
      },
      mainToolbar: {
        description: "Main toolbar.",
        $ref: "#/$defs/workspace.get.MainToolbar",
      },
      fullScreen: {
        description: "Full-screen arrangement (same shape without `fullScreen`).",
        $ref: "#/$defs/workspace.get.Arrangement",
      },
    },
    additionalProperties: false,
    required: [
      "centralPanel",
      "corners",
      "docks",
      "mainToolbar",
      "fullScreen",
    ],
  },
  "workspace.import.ImportOnExists": {
    description: "Import conflict policy.",
    oneOf: [
      {
        description: "Fail.",
        type: "string",
        const: "fail",
      },
      {
        description: "Overwrite.",
        type: "string",
        const: "overwrite",
      },
      {
        description: "Rename the imported workspace.",
        type: "string",
        const: "rename",
      },
    ],
  },
  "workspace.import.params": {
    title: "ImportParams",
    description: "`workspace.import` parameters.",
    type: "object",
    properties: {
      path: {
        description: "Absolute source path.",
        type: "string",
      },
      onExists: {
        description: "Name-conflict policy.",
        $ref: "#/$defs/workspace.import.ImportOnExists",
      },
    },
    additionalProperties: false,
    required: [
      "path",
    ],
  },
  "workspace.import.result": {
    title: "NameResult",
    description: "`{name}`.",
    type: "object",
    properties: {
      name: {
        description: "Workspace name.",
        type: "string",
      },
    },
    required: [
      "name",
    ],
  },
  "workspace.layout.set.Layout": {
    description: "A workspace layout on the wire (01 §12.4 `Layout`): the normal arrangement's fields plus the separate\nfull-screen arrangement (01 §5.6). Panels not mentioned are hidden.",
    type: "object",
    properties: {
      centralPanel: {
        description: "The central view.",
        type: "string",
      },
      corners: {
        description: "Corner ownership.",
        $ref: "#/$defs/workspace.layout.set.Corners",
      },
      docks: {
        description: "Dock areas.",
        $ref: "#/$defs/workspace.layout.set.Docks",
      },
      floating: {
        description: "Floating panels.",
        type: "array",
        items: {
          $ref: "#/$defs/workspace.layout.set.Floating",
        },
        default: [],
      },
      toolPalettes: {
        description: "Tool palettes.",
        type: "array",
        items: {
          $ref: "#/$defs/workspace.layout.set.ToolPalette",
        },
        default: [],
      },
      mainToolbar: {
        description: "Main toolbar.",
        $ref: "#/$defs/workspace.layout.set.MainToolbar",
      },
      fullScreen: {
        description: "Full-screen arrangement (same shape without `fullScreen`).",
        $ref: "#/$defs/workspace.layout.set.Arrangement",
      },
    },
    additionalProperties: false,
    required: [
      "centralPanel",
      "corners",
      "docks",
      "mainToolbar",
      "fullScreen",
    ],
  },
  "workspace.layout.set.Corners": {
    description: "Corner ownership (01 §5.4 *Extend side panels*).",
    oneOf: [
      {
        description: "Left and right areas run full height.",
        type: "string",
        const: "sides",
      },
      {
        description: "Top and bottom areas run full width.",
        type: "string",
        const: "topBottom",
      },
    ],
  },
  "workspace.layout.set.Docks": {
    description: "The four dock areas, each an ordered stack of tab groups.",
    type: "object",
    properties: {
      left: {
        description: "Left area.",
        type: "array",
        items: {
          $ref: "#/$defs/workspace.layout.set.TabGroup",
        },
        default: [],
      },
      right: {
        description: "Right area.",
        type: "array",
        items: {
          $ref: "#/$defs/workspace.layout.set.TabGroup",
        },
        default: [],
      },
      top: {
        description: "Top area.",
        type: "array",
        items: {
          $ref: "#/$defs/workspace.layout.set.TabGroup",
        },
        default: [],
      },
      bottom: {
        description: "Bottom area.",
        type: "array",
        items: {
          $ref: "#/$defs/workspace.layout.set.TabGroup",
        },
        default: [],
      },
    },
    additionalProperties: false,
  },
  "workspace.layout.set.TabGroup": {
    description: "One tab group of a dock area.",
    type: "object",
    properties: {
      tabs: {
        description: "Panel ids, in tab order.",
        type: "array",
        items: {
          type: "string",
        },
      },
      current: {
        description: "The tab in front.",
        type: "string",
      },
      size: {
        description: "On the first group of an area: the area's width (left/right) or height (top/bottom).",
        type: [
          "integer",
          "null",
        ],
        maximum: 2147483647,
        minimum: 0,
      },
    },
    additionalProperties: false,
    required: [
      "tabs",
      "current",
    ],
  },
  "workspace.layout.set.Floating": {
    description: "A floating panel.",
    type: "object",
    properties: {
      panelId: {
        description: "Panel id.",
        type: "string",
      },
      visible: {
        description: "Shown.",
        type: "boolean",
      },
      geometry: {
        description: "Window geometry.",
        $ref: "#/$defs/workspace.layout.set.Geometry",
      },
    },
    additionalProperties: false,
    required: [
      "panelId",
      "visible",
      "geometry",
    ],
  },
  "workspace.layout.set.Geometry": {
    description: "A geometry in logical pixels.",
    type: "object",
    properties: {
      x: {
        description: "Left.",
        type: "integer",
        maximum: 2147483647,
        minimum: -2147483648,
      },
      y: {
        description: "Top.",
        type: "integer",
        maximum: 2147483647,
        minimum: -2147483648,
      },
      width: {
        description: "Width.",
        type: "integer",
        maximum: 32767,
        minimum: 1,
      },
      height: {
        description: "Height.",
        type: "integer",
        maximum: 32767,
        minimum: 1,
      },
    },
    additionalProperties: false,
    required: [
      "x",
      "y",
      "width",
      "height",
    ],
  },
  "workspace.layout.set.ToolPalette": {
    description: "A tool palette.",
    type: "object",
    properties: {
      id: {
        description: "Palette id (`drawingTools`, …).",
        type: "string",
      },
      visible: {
        description: "Shown.",
        type: "boolean",
      },
      floating: {
        description: "Floating (else docked with the main toolbar).",
        type: "boolean",
      },
      position: {
        description: "Floating position.",
        anyOf: [
          {
            $ref: "#/$defs/workspace.layout.set.Position",
          },
          {
            type: "null",
          },
        ],
      },
    },
    additionalProperties: false,
    required: [
      "id",
      "visible",
      "floating",
    ],
  },
  "workspace.layout.set.Position": {
    description: "A point in logical pixels.",
    type: "object",
    properties: {
      x: {
        description: "Left.",
        type: "integer",
        maximum: 2147483647,
        minimum: -2147483648,
      },
      y: {
        description: "Top.",
        type: "integer",
        maximum: 2147483647,
        minimum: -2147483648,
      },
    },
    additionalProperties: false,
    required: [
      "x",
      "y",
    ],
  },
  "workspace.layout.set.MainToolbar": {
    description: "The main toolbar configuration (01 §5.7).",
    type: "object",
    properties: {
      visible: {
        description: "Shown.",
        type: "boolean",
      },
      items: {
        description: "Items: command references, group ids, `separator`, `space`, `flexibleSpace`.",
        type: "array",
        items: {
          type: "string",
        },
      },
    },
    additionalProperties: false,
    required: [
      "visible",
      "items",
    ],
  },
  "workspace.layout.set.Arrangement": {
    description: "An arrangement of the main window (01 §12.4 `Layout`, without the nested full-screen arrangement).",
    type: "object",
    properties: {
      centralPanel: {
        description: "The central view.",
        type: "string",
      },
      corners: {
        description: "Corner ownership.",
        $ref: "#/$defs/workspace.layout.set.Corners",
      },
      docks: {
        description: "Dock areas.",
        $ref: "#/$defs/workspace.layout.set.Docks",
      },
      floating: {
        description: "Floating panels.",
        type: "array",
        items: {
          $ref: "#/$defs/workspace.layout.set.Floating",
        },
        default: [],
      },
      toolPalettes: {
        description: "Tool palettes.",
        type: "array",
        items: {
          $ref: "#/$defs/workspace.layout.set.ToolPalette",
        },
        default: [],
      },
      mainToolbar: {
        description: "Main toolbar.",
        $ref: "#/$defs/workspace.layout.set.MainToolbar",
      },
    },
    additionalProperties: false,
    required: [
      "centralPanel",
      "corners",
      "docks",
      "mainToolbar",
    ],
  },
  "workspace.layout.set.params": {
    title: "LayoutParams",
    description: "`{layout}` parameters.",
    type: "object",
    properties: {
      layout: {
        description: "The layout to apply (not saved).",
        $ref: "#/$defs/workspace.layout.set.Layout",
      },
    },
    additionalProperties: false,
    required: [
      "layout",
    ],
  },
  "workspace.layout.set.result": {
    title: "Layout",
    description: "A workspace layout on the wire (01 §12.4 `Layout`): the normal arrangement's fields plus the separate\nfull-screen arrangement (01 §5.6). Panels not mentioned are hidden.",
    type: "object",
    properties: {
      centralPanel: {
        description: "The central view.",
        type: "string",
      },
      corners: {
        description: "Corner ownership.",
        $ref: "#/$defs/workspace.layout.set.Corners",
      },
      docks: {
        description: "Dock areas.",
        $ref: "#/$defs/workspace.layout.set.Docks",
      },
      floating: {
        description: "Floating panels.",
        type: "array",
        items: {
          $ref: "#/$defs/workspace.layout.set.Floating",
        },
        default: [],
      },
      toolPalettes: {
        description: "Tool palettes.",
        type: "array",
        items: {
          $ref: "#/$defs/workspace.layout.set.ToolPalette",
        },
        default: [],
      },
      mainToolbar: {
        description: "Main toolbar.",
        $ref: "#/$defs/workspace.layout.set.MainToolbar",
      },
      fullScreen: {
        description: "Full-screen arrangement (same shape without `fullScreen`).",
        $ref: "#/$defs/workspace.layout.set.Arrangement",
      },
    },
    additionalProperties: false,
    required: [
      "centralPanel",
      "corners",
      "docks",
      "mainToolbar",
      "fullScreen",
    ],
  },
  "workspace.list.params": {
    title: "NoParams",
    description: "No parameters.",
    type: "object",
    additionalProperties: false,
  },
  "workspace.list.WorkspaceEntry": {
    description: "A workspace as listed by `workspace.list`.",
    type: "object",
    properties: {
      name: {
        description: "Name.",
        type: "string",
      },
      builtIn: {
        description: "Built in (cannot be deleted).",
        type: "boolean",
      },
      modified: {
        description: "The current arrangement differs from the stored one (current workspace only).",
        type: "boolean",
      },
      position: {
        description: "Position in the list (1-based).",
        type: "integer",
        format: "uint",
        minimum: 0,
      },
      current: {
        description: "The current workspace.",
        type: "boolean",
      },
    },
    required: [
      "name",
      "builtIn",
      "modified",
      "position",
      "current",
    ],
  },
  "workspace.list.result": {
    title: "Array_of_WorkspaceEntry",
    type: "array",
    items: {
      $ref: "#/$defs/workspace.list.WorkspaceEntry",
    },
  },
  "workspace.rename.params": {
    title: "RenameParams",
    description: "`workspace.rename` parameters.",
    type: "object",
    properties: {
      name: {
        description: "Current name.",
        type: "string",
      },
      newName: {
        description: "New name.",
        type: "string",
      },
    },
    additionalProperties: false,
    required: [
      "name",
      "newName",
    ],
  },
  "workspace.rename.result": {
    title: "NameResult",
    description: "`{name}`.",
    type: "object",
    properties: {
      name: {
        description: "Workspace name.",
        type: "string",
      },
    },
    required: [
      "name",
    ],
  },
  "workspace.reset.params": {
    title: "ResetParams",
    description: "`workspace.reset` parameters.",
    type: "object",
    properties: {
      name: {
        description: "Built-in workspace (default: current).",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
      all: {
        description: "Every built-in workspace.",
        type: "boolean",
        default: false,
      },
      select: {
        description: "Select it afterwards.",
        type: "boolean",
        default: false,
      },
    },
    additionalProperties: false,
  },
  "workspace.reset.result": {
    title: "ResetResult",
    description: "`{reset}`.",
    type: "object",
    properties: {
      reset: {
        description: "Reset workspaces.",
        type: "array",
        items: {
          type: "string",
        },
      },
    },
    required: [
      "reset",
    ],
  },
  "workspace.save.params": {
    title: "OptionalName",
    description: "`{name?}` parameters.",
    type: "object",
    properties: {
      name: {
        description: "Workspace name (default: the current arrangement).",
        type: [
          "string",
          "null",
        ],
        default: null,
      },
    },
    additionalProperties: false,
  },
  "workspace.save.result": {
    title: "NameResult",
    description: "`{name}`.",
    type: "object",
    properties: {
      name: {
        description: "Workspace name.",
        type: "string",
      },
    },
    required: [
      "name",
    ],
  },
  "workspace.select.params": {
    title: "SelectParams",
    description: "`workspace.select` parameters.",
    type: "object",
    properties: {
      name: {
        description: "Workspace name.",
        type: "string",
      },
      saveCurrent: {
        description: "Save the current arrangement into the current workspace first (default: `general.workspace.autoSave`,\nwhich is off until preferences arrive in M4).",
        type: [
          "boolean",
          "null",
        ],
        default: null,
      },
    },
    additionalProperties: false,
    required: [
      "name",
    ],
  },
  "workspace.select.result": {
    title: "NameResult",
    description: "`{name}`.",
    type: "object",
    properties: {
      name: {
        description: "Workspace name.",
        type: "string",
      },
    },
    required: [
      "name",
    ],
  },
};

/** `status` of an envelope. */
export type envelope_Status = 'ok' | 'needs_confirmation' | 'error';

/** One object change of a committed (or dry-run) transaction (10 §2.6, 11 §7.2). */
export interface envelope_ChangeRecord {
  /** Object id (compound for container-local ids). */
  id: string;
  /** Object type. */
  type: string;
  /** `created`, `modified` or `deleted`. */
  op: string;
  /** The object's `rev` after the transaction (the `documentRev` it would get for a dry run). */
  rev: number | bigint;
  /** Parent (created and deleted objects). */
  parentId?: string | null;
  /** Position in the parent's collection (created objects). */
  index?: number | bigint | null;
  /** Own-view RFC 6902 patch with `oldValue` (modified objects). */
  patch?: unknown[] | null;
}

/** The undo step a transaction created. */
export interface envelope_UndoRef {
  /** Step id (`u-<n>`). */
  stepId: string;
  /** Label. */
  label: string;
}

/** A warning (11 §6.3, §10): same code space as errors, never aborts. */
export interface envelope_Warning {
  /** Code. */
  code: string;
  /** Message. */
  message: string;
  /** Affected objects. */
  objectIds?: string[];
}

/** A typed command error in the `{code, message, details, hint?}` wire form (11 §10). */
export interface envelope_CommandError {
  /** Stable dotted code `area.reason`. */
  code: string;
  /** Human-readable message. */
  message: string;
  /** Structured details (boxed: the error travels in every `Result` of the command path, so it stays small). */
  details: unknown;
  /** How to fix the call, where known. */
  hint?: string | null;
}

/** The command result envelope (11 §7.2), also the shape of each batch step. */
export interface envelope_root {
  /** `ok`, `needs_confirmation` or `error`. */
  status: envelope_Status;
  /** Command-specific result (per its result schema). */
  result?: unknown;
  /** What was actually targeted (P4). */
  targets?: unknown;
  /** Changed objects (capped at [`crate::CHANGES_CAP`]). */
  changes: envelope_ChangeRecord[];
  /** `true` when `changes` was capped; read the rest with `doc.diff` from `changesSince`. */
  changesTruncated?: boolean;
  /** Revision to pass to `doc.diff` when `changes` was capped. */
  changesSince?: number | bigint | null;
  /** The undo step created. */
  undo?: envelope_UndoRef | null;
  /** Warnings. */
  warnings: envelope_Warning[];
  /** `documentRev` after the call (`null` with no project). */
  documentRev?: number | bigint | null;
  /** The error of a failed call. */
  error?: envelope_CommandError | null;
  /** `true` for a dry run. */
  dryRun?: boolean;
  /** Batch step envelopes. */
  steps?: envelope_root[] | null;
}

/** Export format. */
export type agent_activity_export_ActivityFormat = 'jsonl' | 'batch';

/** Export conflict policy. */
export type agent_activity_export_ExportOnExists = 'fail' | 'overwrite' | 'rename';

/** `agent.activity.export` parameters. */
export interface agent_activity_export_params {
  /** Target path. */
  path: string;
  /** Format. */
  format?: agent_activity_export_ActivityFormat;
  /** Conflict policy. */
  onExists?: agent_activity_export_ExportOnExists;
}

/** `{path, entries}`. */
export interface agent_activity_export_result {
  /** File written. */
  path: string;
  /** Entries. */
  entries: number;
}

/** `agent.activity.list` parameters. */
export interface agent_activity_list_params {
  /** Session. */
  sessionId?: string | null;
  /** Since (ISO 8601). */
  since?: string | null;
  /** Maximum entries. */
  limit?: number | null;
}

export type agent_activity_list_result = (Record<string, unknown>)[];

/** `{enabled}`. */
export interface agent_activity_persist_set_params {
  /** Persist the activity log. */
  enabled: boolean;
}

/** `{enabled}`. */
export interface agent_activity_persist_set_result {
  /** Persist the activity log. */
  enabled: boolean;
}

/** No parameters. */
export type agent_confirmation_requested_params = Record<string, unknown>;

export type agent_confirmation_requested_result = Record<string, unknown>;

/** `agent.confirmation.respond` parameters. */
export interface agent_confirmation_respond_params {
  /** Confirmation. */
  confirmationId: string;
  /** Accept or decline. */
  accept: boolean;
}

/** `{confirmationId, accepted}`. */
export interface agent_confirmation_respond_result {
  /** Confirmation. */
  confirmationId: string;
  /** Accepted. */
  accepted: boolean;
}

/** Policy presets (11 §6.1). */
export type agent_policy_defaults_set_Policy = 'observe' | 'assist' | 'autonomous';

/** `agent.policy.defaults.set` parameters. */
export interface agent_policy_defaults_set_params {
  /** Default policy. */
  policy?: agent_policy_defaults_set_Policy | null;
  /** Concurrent jobs per session. */
  maxJobs?: number | null;
  /** Steps per batch. */
  maxBatchSteps?: number | null;
  /** Largest inline image side. */
  maxInlineImagePx?: number | null;
  /** Largest inline result. */
  maxInlineBytes?: number | null;
  /** Ask in the app. */
  askInApp?: boolean | null;
  /** Let agents move the view. */
  letAgentMoveView?: boolean | null;
  /** Highlight agent changes. */
  highlightAgentChanges?: boolean | null;
}

export type agent_policy_defaults_set_result = Record<string, unknown>;

/** `agent.server.set` parameters. */
export interface agent_server_set_params {
  /** Enable the server. */
  enabled: boolean;
  /** Streamable HTTP transport. */
  http?: boolean | null;
  /** Port. */
  port?: number | null;
  /** Accept non-loopback connections. */
  allowRemote?: boolean | null;
}

/** `agent.server.set` result. */
export interface agent_server_set_result {
  /** Enabled. */
  enabled: boolean;
  /** Endpoint URL. */
  endpoint?: string | null;
  /** Discovery file path. */
  discoveryFile?: string | null;
}

/** No parameters. */
export type agent_server_status_params = Record<string, unknown>;

export type agent_server_status_result = Record<string, unknown>;

/** No parameters. */
export type agent_session_connected_params = Record<string, unknown>;

export type agent_session_connected_result = Record<string, unknown>;

/** `{sessionId}` parameters. */
export interface agent_session_disconnect_params {
  /** Session. */
  sessionId: string;
}

/** An empty result object. */
export type agent_session_disconnect_result = Record<string, unknown>;

/** No parameters. */
export type agent_session_disconnected_params = Record<string, unknown>;

export type agent_session_disconnected_result = Record<string, unknown>;

/** Policy presets (11 §6.1). */
export type agent_session_grant_Policy = 'observe' | 'assist' | 'autonomous';

/** `agent.session.grant` parameters. */
export interface agent_session_grant_params {
  /** Session. */
  sessionId: string;
  /** Policy preset. */
  policy?: agent_session_grant_Policy | null;
  /** Scopes. */
  scopes?: string[] | null;
  /** File-system roots. */
  roots?: string[] | null;
}

export type agent_session_grant_result = Record<string, unknown>;

/** `{sessionId?}` parameters. */
export interface agent_session_pause_params {
  /** Session (default: all). */
  sessionId?: string | null;
}

/** `{paused}`. */
export interface agent_session_pause_result {
  /** Paused sessions. */
  paused: string[];
}

/** `{sessionId?}` parameters. */
export interface agent_session_resume_params {
  /** Session (default: all). */
  sessionId?: string | null;
}

/** `{resumed}`. */
export interface agent_session_resume_result {
  /** Resumed sessions. */
  resumed: string[];
}

/** No parameters. */
export type agent_token_regenerate_params = Record<string, unknown>;

/** `{discoveryFile}`. */
export interface agent_token_regenerate_result {
  /** Discovery file path. */
  discoveryFile: string;
}

/** `agent.typedTools.set` parameters. */
export interface agent_typedTools_set_params {
  /** Expose one MCP tool per command. */
  enabled: boolean;
  /** Limit to namespaces. */
  namespaces?: string[] | null;
}

/** `{toolCount}`. */
export interface agent_typedTools_set_result {
  /** Tools exposed. */
  toolCount: number;
}

/** `app.context.set` parameters. */
export interface app_context_set_params {
  /** Scene to make current (id or path). */
  sceneId?: string | null;
  /** Frame (1-based). */
  frame?: number | null;
  /** Element to make current (must belong to the scene). */
  elementId?: string | null;
  /** Drawing to make current (must belong to the element's drawing bank). */
  drawingId?: string | null;
}

/** The open project in `AppState`. */
export interface app_context_set_ProjectRef {
  /** Project id. */
  id: string;
  /** Name. */
  name: string;
  /** File path (`null` for an unsaved project). */
  path?: string | null;
  /** Modified since the last save. */
  dirty: boolean;
  /** The global library has unsaved changes (08; always `false` until M9). */
  globalLibraryDirty: boolean;
  /** Opened read-only. */
  readOnly: boolean;
}

/** `{id, name, frameCount}` of a scene. */
export interface app_context_set_SceneRef {
  /** Scene id. */
  id: string;
  /** Name. */
  name: string;
  /** Scene length (derived, 10 §4.3). */
  frameCount: number | bigint;
}

/** `{id, name, type}` of an element. */
export interface app_context_set_ElementRef {
  /** Element id. */
  id: string;
  /** Name. */
  name: string;
  /** Element type (01 §12.4 reports `elementType` as `type`). */
  type: string;
}

/** `{id, name}` of a drawing. */
export interface app_context_set_DrawingRef {
  /** Drawing id. */
  id: string;
  /** Name. */
  name: string;
}

/** The working mode (01 §4.4). */
export type app_context_set_Mode = 'drawing' | 'staging';

/** The active view. */
export interface app_context_set_ActiveView {
  /** View id (the kernel has one view per view panel; ids equal panel ids until M4). */
  viewId: string;
  /** Panel id. */
  panelId: string;
  /** `camera`, `drawing`, `top`, `side`. */
  type: string;
}

/** The active tool (M4 C4.2 selects tools; `null` until then). */
export interface app_context_set_ActiveTool {
  /** Tool id. */
  toolId: string;
}

/** The selection in `AppState`. */
export interface app_context_set_SelectionState {
  /** Selection context. */
  context: string;
  /** Selected ids. */
  ids: string[];
  /** Object types present in the selection. */
  kinds: string[];
}

/** The workspace in `AppState`. */
export interface app_context_set_WorkspaceState {
  /** Current workspace. */
  name: string;
  /** The arrangement differs from the stored one. */
  modified: boolean;
}

/** Playback in `AppState` (M5 drives it; the kernel reports stopped). */
export interface app_context_set_PlaybackState {
  /** Playing. */
  playing: boolean;
}

/** A modal GUI state (never set by the kernel: 11 P5). */
export interface app_context_set_Modal {
  /** `dialog`, `gesture` or `progress`. */
  kind: string;
  /** Title. */
  title: string;
}

/** An agent session in `AppState` (C1.4 fills the list). */
export interface app_context_set_AgentSessionState {
  /** Session id. */
  sessionId: string;
  /** Client display name. */
  name: string;
  /** Currently running a command. */
  acting: boolean;
}

/** `AppState` (01 §12.4). */
export interface app_context_set_result {
  /** Open project or `null`. */
  project?: app_context_set_ProjectRef | null;
  /** Current scene. */
  scene?: app_context_set_SceneRef | null;
  /** Current frame (`null` without a scene). */
  frame?: number | null;
  /** Current element. */
  element?: app_context_set_ElementRef | null;
  /** Current drawing. */
  drawing?: app_context_set_DrawingRef | null;
  /** Working mode (`null` without a project). */
  mode?: app_context_set_Mode | null;
  /** Active view. */
  activeView?: app_context_set_ActiveView | null;
  /** Focused panel. */
  focusedPanel?: string | null;
  /** Active tool. */
  activeTool?: app_context_set_ActiveTool | null;
  /** Selection. */
  selection: app_context_set_SelectionState;
  /** Workspace. */
  workspace: app_context_set_WorkspaceState;
  /** Playback. */
  playback: app_context_set_PlaybackState;
  /** Start screen shown (no project). */
  startScreenVisible: boolean;
  /** Modal state (always `null` in the kernel). */
  modal?: app_context_set_Modal | null;
  /** Connected agent sessions. */
  agentSessions: app_context_set_AgentSessionState[];
  /** Full-screen presentation (01 §5.6; explicit `enabled` state, readiness bullet 4). */
  fullScreen: boolean;
}

/** Previous / next. */
export type app_currentElement_move_Direction = 'previous' | 'next';

/** Element order for `app.currentElement.move`. */
export type app_currentElement_move_ElementOrder = 'auto' | 'timeline' | 'xsheet';

/** `app.currentElement.move` parameters. */
export interface app_currentElement_move_params {
  /** Previous or next. */
  direction: app_currentElement_move_Direction;
  /** Which order to follow. */
  order?: app_currentElement_move_ElementOrder;
}

/** `{elementId}`. */
export interface app_currentElement_move_result {
  /** The current element (`null` when the scene has none). */
  elementId?: string | null;
}

/** `app.currentElement.set` parameters. */
export interface app_currentElement_set_params {
  /** The element (id or path); its scene becomes current. */
  elementId: string;
}

/** `{elementId}`. */
export interface app_currentElement_set_result {
  /** The current element (`null` when the scene has none). */
  elementId?: string | null;
}

/** Previous / next. */
export type app_frame_move_Direction = 'previous' | 'next';

/** Step unit for `app.frame.move`. */
export type app_frame_move_FrameUnit = 'frame' | 'drawing';

/** `app.frame.move` parameters. */
export interface app_frame_move_params {
  /** Previous or next. */
  direction: app_frame_move_Direction;
  /** Frames or drawings. */
  unit?: app_frame_move_FrameUnit;
  /** How many steps. */
  count?: number;
}

/** `{frame}`. */
export interface app_frame_move_result {
  /** The current frame. */
  frame: number;
}

/** `app.frame.set` parameters. */
export interface app_frame_set_params {
  /** Frame, 1 to the scene length. */
  frame: number;
}

/** `{frame}`. */
export interface app_frame_set_result {
  /** The current frame. */
  frame: number;
}

/** No parameters. */
export type app_info_get_params = Record<string, unknown>;

/** `app.info.get` result. */
export interface app_info_get_result {
  /** Application name. */
  name: string;
  /** Version. */
  version: string;
  /** Platform. */
  platform: string;
  /** UI locale. */
  locale: string;
  /** Available UI languages. */
  languages: string[];
  /** Project format version. */
  formatVersion: string;
}

/** Working mode parameter. */
export type app_mode_set_ModeParam = 'drawing' | 'staging';

/** `app.mode.set` parameters. */
export interface app_mode_set_params {
  /** Drawing or staging. */
  mode: app_mode_set_ModeParam;
  /** The view to switch (a view panel id); default the central view. */
  viewId?: string | null;
}

/** The working mode (01 §4.4). */
export type app_mode_set_Mode = 'drawing' | 'staging';

/** `{mode, viewId}`. */
export interface app_mode_set_result {
  /** The mode. */
  mode: app_mode_set_Mode;
  /** The view. */
  viewId: string;
}

/** What to do with unsaved changes (01 §11.2 rule 3). */
export type app_quit_OnUnsaved = 'fail' | 'save' | 'discard';

/** `app.quit` parameters. */
export interface app_quit_params {
  /** Unsaved-changes policy. */
  onUnsaved?: app_quit_OnUnsaved;
}

/** `{quitting}`. */
export interface app_quit_result {
  /** The application quits. */
  quitting: boolean;
}

/** No parameters. */
export type app_state_changed_params = Record<string, unknown>;

/** `AppState` (01 §12.4). */
export interface app_state_changed_AppState {
  /** Open project or `null`. */
  project?: app_state_changed_ProjectRef | null;
  /** Current scene. */
  scene?: app_state_changed_SceneRef | null;
  /** Current frame (`null` without a scene). */
  frame?: number | null;
  /** Current element. */
  element?: app_state_changed_ElementRef | null;
  /** Current drawing. */
  drawing?: app_state_changed_DrawingRef | null;
  /** Working mode (`null` without a project). */
  mode?: app_state_changed_Mode | null;
  /** Active view. */
  activeView?: app_state_changed_ActiveView | null;
  /** Focused panel. */
  focusedPanel?: string | null;
  /** Active tool. */
  activeTool?: app_state_changed_ActiveTool | null;
  /** Selection. */
  selection: app_state_changed_SelectionState;
  /** Workspace. */
  workspace: app_state_changed_WorkspaceState;
  /** Playback. */
  playback: app_state_changed_PlaybackState;
  /** Start screen shown (no project). */
  startScreenVisible: boolean;
  /** Modal state (always `null` in the kernel). */
  modal?: app_state_changed_Modal | null;
  /** Connected agent sessions. */
  agentSessions: app_state_changed_AgentSessionState[];
  /** Full-screen presentation (01 §5.6; explicit `enabled` state, readiness bullet 4). */
  fullScreen: boolean;
}

/** The open project in `AppState`. */
export interface app_state_changed_ProjectRef {
  /** Project id. */
  id: string;
  /** Name. */
  name: string;
  /** File path (`null` for an unsaved project). */
  path?: string | null;
  /** Modified since the last save. */
  dirty: boolean;
  /** The global library has unsaved changes (08; always `false` until M9). */
  globalLibraryDirty: boolean;
  /** Opened read-only. */
  readOnly: boolean;
}

/** `{id, name, frameCount}` of a scene. */
export interface app_state_changed_SceneRef {
  /** Scene id. */
  id: string;
  /** Name. */
  name: string;
  /** Scene length (derived, 10 §4.3). */
  frameCount: number | bigint;
}

/** `{id, name, type}` of an element. */
export interface app_state_changed_ElementRef {
  /** Element id. */
  id: string;
  /** Name. */
  name: string;
  /** Element type (01 §12.4 reports `elementType` as `type`). */
  type: string;
}

/** `{id, name}` of a drawing. */
export interface app_state_changed_DrawingRef {
  /** Drawing id. */
  id: string;
  /** Name. */
  name: string;
}

/** The working mode (01 §4.4). */
export type app_state_changed_Mode = 'drawing' | 'staging';

/** The active view. */
export interface app_state_changed_ActiveView {
  /** View id (the kernel has one view per view panel; ids equal panel ids until M4). */
  viewId: string;
  /** Panel id. */
  panelId: string;
  /** `camera`, `drawing`, `top`, `side`. */
  type: string;
}

/** The active tool (M4 C4.2 selects tools; `null` until then). */
export interface app_state_changed_ActiveTool {
  /** Tool id. */
  toolId: string;
}

/** The selection in `AppState`. */
export interface app_state_changed_SelectionState {
  /** Selection context. */
  context: string;
  /** Selected ids. */
  ids: string[];
  /** Object types present in the selection. */
  kinds: string[];
}

/** The workspace in `AppState`. */
export interface app_state_changed_WorkspaceState {
  /** Current workspace. */
  name: string;
  /** The arrangement differs from the stored one. */
  modified: boolean;
}

/** Playback in `AppState` (M5 drives it; the kernel reports stopped). */
export interface app_state_changed_PlaybackState {
  /** Playing. */
  playing: boolean;
}

/** A modal GUI state (never set by the kernel: 11 P5). */
export interface app_state_changed_Modal {
  /** `dialog`, `gesture` or `progress`. */
  kind: string;
  /** Title. */
  title: string;
}

/** An agent session in `AppState` (C1.4 fills the list). */
export interface app_state_changed_AgentSessionState {
  /** Session id. */
  sessionId: string;
  /** Client display name. */
  name: string;
  /** Currently running a command. */
  acting: boolean;
}

/** `app.state.changed` payload. */
export interface app_state_changed_result {
  /** Top-level `AppState` fields that changed. */
  changed: string[];
  /** The new state. */
  state: app_state_changed_AppState;
}

/** No parameters. */
export type app_state_get_params = Record<string, unknown>;

/** The open project in `AppState`. */
export interface app_state_get_ProjectRef {
  /** Project id. */
  id: string;
  /** Name. */
  name: string;
  /** File path (`null` for an unsaved project). */
  path?: string | null;
  /** Modified since the last save. */
  dirty: boolean;
  /** The global library has unsaved changes (08; always `false` until M9). */
  globalLibraryDirty: boolean;
  /** Opened read-only. */
  readOnly: boolean;
}

/** `{id, name, frameCount}` of a scene. */
export interface app_state_get_SceneRef {
  /** Scene id. */
  id: string;
  /** Name. */
  name: string;
  /** Scene length (derived, 10 §4.3). */
  frameCount: number | bigint;
}

/** `{id, name, type}` of an element. */
export interface app_state_get_ElementRef {
  /** Element id. */
  id: string;
  /** Name. */
  name: string;
  /** Element type (01 §12.4 reports `elementType` as `type`). */
  type: string;
}

/** `{id, name}` of a drawing. */
export interface app_state_get_DrawingRef {
  /** Drawing id. */
  id: string;
  /** Name. */
  name: string;
}

/** The working mode (01 §4.4). */
export type app_state_get_Mode = 'drawing' | 'staging';

/** The active view. */
export interface app_state_get_ActiveView {
  /** View id (the kernel has one view per view panel; ids equal panel ids until M4). */
  viewId: string;
  /** Panel id. */
  panelId: string;
  /** `camera`, `drawing`, `top`, `side`. */
  type: string;
}

/** The active tool (M4 C4.2 selects tools; `null` until then). */
export interface app_state_get_ActiveTool {
  /** Tool id. */
  toolId: string;
}

/** The selection in `AppState`. */
export interface app_state_get_SelectionState {
  /** Selection context. */
  context: string;
  /** Selected ids. */
  ids: string[];
  /** Object types present in the selection. */
  kinds: string[];
}

/** The workspace in `AppState`. */
export interface app_state_get_WorkspaceState {
  /** Current workspace. */
  name: string;
  /** The arrangement differs from the stored one. */
  modified: boolean;
}

/** Playback in `AppState` (M5 drives it; the kernel reports stopped). */
export interface app_state_get_PlaybackState {
  /** Playing. */
  playing: boolean;
}

/** A modal GUI state (never set by the kernel: 11 P5). */
export interface app_state_get_Modal {
  /** `dialog`, `gesture` or `progress`. */
  kind: string;
  /** Title. */
  title: string;
}

/** An agent session in `AppState` (C1.4 fills the list). */
export interface app_state_get_AgentSessionState {
  /** Session id. */
  sessionId: string;
  /** Client display name. */
  name: string;
  /** Currently running a command. */
  acting: boolean;
}

/** `AppState` (01 §12.4). */
export interface app_state_get_result {
  /** Open project or `null`. */
  project?: app_state_get_ProjectRef | null;
  /** Current scene. */
  scene?: app_state_get_SceneRef | null;
  /** Current frame (`null` without a scene). */
  frame?: number | null;
  /** Current element. */
  element?: app_state_get_ElementRef | null;
  /** Current drawing. */
  drawing?: app_state_get_DrawingRef | null;
  /** Working mode (`null` without a project). */
  mode?: app_state_get_Mode | null;
  /** Active view. */
  activeView?: app_state_get_ActiveView | null;
  /** Focused panel. */
  focusedPanel?: string | null;
  /** Active tool. */
  activeTool?: app_state_get_ActiveTool | null;
  /** Selection. */
  selection: app_state_get_SelectionState;
  /** Workspace. */
  workspace: app_state_get_WorkspaceState;
  /** Playback. */
  playback: app_state_get_PlaybackState;
  /** Start screen shown (no project). */
  startScreenVisible: boolean;
  /** Modal state (always `null` in the kernel). */
  modal?: app_state_get_Modal | null;
  /** Connected agent sessions. */
  agentSessions: app_state_get_AgentSessionState[];
  /** Full-screen presentation (01 §5.6; explicit `enabled` state, readiness bullet 4). */
  fullScreen: boolean;
}

/** `{name}` parameters. */
export interface checkpoint_create_params {
  /** Checkpoint name. */
  name: string;
}

/** `{name, documentRev}`. */
export interface checkpoint_create_result {
  /** Name. */
  name: string;
  /** The document revision it holds. */
  documentRev: number | bigint;
}

/** `{name}` parameters. */
export interface checkpoint_delete_params {
  /** Checkpoint name. */
  name: string;
}

/** An empty result object. */
export type checkpoint_delete_result = Record<string, unknown>;

/** No parameters. */
export type checkpoint_list_params = Record<string, unknown>;

/** A checkpoint as listed. */
export interface checkpoint_list_CheckpointEntry {
  /** Name. */
  name: string;
  /** Creation time. */
  createdAt: string;
  /** Document revision. */
  documentRev: number | bigint;
  /** Who created it. */
  origin: string;
}

export type checkpoint_list_result = checkpoint_list_CheckpointEntry[];

/** `{name}` parameters. */
export interface checkpoint_restore_params {
  /** Checkpoint name. */
  name: string;
}

/** `{name, documentRev}`. */
export interface checkpoint_restore_result {
  /** Name. */
  name: string;
  /** The document revision it holds. */
  documentRev: number | bigint;
}

/** `doc.asset.embed` parameters. */
export interface doc_asset_embed_params {
  /** Assets. */
  assetIds?: string[] | null;
  /** Every linked asset. */
  all?: boolean;
}

export type doc_asset_embed_result = Record<string, unknown>;

/** What to do when a target file exists (01 §11.2 rule 4). */
export type doc_asset_export_OnExists = 'fail' | 'overwrite';

/** `doc.asset.export` parameters. */
export interface doc_asset_export_params {
  /** Asset. */
  assetId: string;
  /** Target path. */
  path: string;
  /** Overwrite policy. */
  onExists?: doc_asset_export_OnExists;
}

export type doc_asset_export_result = Record<string, unknown>;

/** Asset kind. */
export type doc_asset_list_AssetKind = 'sound' | 'image' | 'movie' | 'vector';

/** Asset status filter. */
export type doc_asset_list_AssetStatus = 'all' | 'missing' | 'changed' | 'linked' | 'embedded' | 'unused';

/** `doc.asset.list` parameters. */
export interface doc_asset_list_params {
  /** Kind. */
  kind?: doc_asset_list_AssetKind | null;
  /** Status. */
  status?: doc_asset_list_AssetStatus;
}

/** `{assets}`. */
export interface doc_asset_list_result {
  /** `{id, name, kind, storage, status, path?, size, refs}`. */
  assets: (Record<string, unknown>)[];
}

/** Relink matching. */
export type doc_asset_relink_MatchBy = 'name' | 'nameAndSize' | 'hash';

/** `doc.asset.relink` parameters. */
export interface doc_asset_relink_params {
  /** Asset. */
  assetId?: string | null;
  /** New path. */
  path?: string | null;
  /** Folder to search. */
  searchFolder?: string | null;
  /** Search subfolders. */
  recursive?: boolean;
  /** Matching. */
  matchBy?: doc_asset_relink_MatchBy;
}

export type doc_asset_relink_result = Record<string, unknown>;

/** `doc.asset.remove` parameters. */
export interface doc_asset_remove_params {
  /** Assets. */
  assetIds?: string[] | null;
  /** Every unused asset. */
  unused?: boolean;
  /** Remove even if referenced. */
  force?: boolean;
}

export type doc_asset_remove_result = Record<string, unknown>;

/** No parameters. */
export type doc_asset_status_changed_params = Record<string, unknown>;

export type doc_asset_status_changed_result = Record<string, unknown>;

/** `doc.blob.get` parameters. */
export interface doc_blob_get_params {
  /** An asset or object holding a blob. */
  ref?: string | null;
  /** A package entry. */
  entry?: string | null;
  /** Size limit. */
  maxBytes?: number;
}

/** `doc.blob.get` result. */
export interface doc_blob_get_result {
  /** Media type. */
  mediaType: string;
  /** Size. */
  size: number | bigint;
  /** SHA-256. */
  sha256: string;
  /** Content. */
  base64: string;
}

/** No parameters. */
export type doc_changed_params = Record<string, unknown>;

/** One object change of a committed (or dry-run) transaction (10 §2.6, 11 §7.2). */
export interface doc_changed_ChangeRecord {
  /** Object id (compound for container-local ids). */
  id: string;
  /** Object type. */
  type: string;
  /** `created`, `modified` or `deleted`. */
  op: string;
  /** The object's `rev` after the transaction (the `documentRev` it would get for a dry run). */
  rev: number | bigint;
  /** Parent (created and deleted objects). */
  parentId?: string | null;
  /** Position in the parent's collection (created objects). */
  index?: number | bigint | null;
  /** Own-view RFC 6902 patch with `oldValue` (modified objects). */
  patch?: unknown[] | null;
}

/** `doc.changed` payload (10 §2.6). */
export interface doc_changed_result {
  /** The step created or replayed. */
  undoStepId: string;
  /** The transaction's revision. */
  documentRev: number | bigint;
  /** Label. */
  label: string;
  /** `user`, `agent:<client>`, `system`, `load`. */
  origin: string;
  /** Changes (values over 64 KiB elided). */
  changes: doc_changed_ChangeRecord[];
}

/** Detail of `doc.diff`. */
export type doc_diff_Detail = 'summary' | 'full';

/** `doc.diff` parameters. */
export interface doc_diff_params {
  /** Base revision. */
  sinceRev?: number | bigint | null;
  /** Base checkpoint (by name). */
  sinceCheckpoint?: string | null;
  /** End revision (default: current). */
  untilRev?: number | bigint | null;
  /** Only objects inside this one. */
  within?: string | null;
  /** Only these types. */
  types?: string[] | null;
  /** With or without patches. */
  detail?: doc_diff_Detail;
  /** Maximum changes. */
  limit?: number;
}

/** One object change of a committed (or dry-run) transaction (10 §2.6, 11 §7.2). */
export interface doc_diff_ChangeRecord {
  /** Object id (compound for container-local ids). */
  id: string;
  /** Object type. */
  type: string;
  /** `created`, `modified` or `deleted`. */
  op: string;
  /** The object's `rev` after the transaction (the `documentRev` it would get for a dry run). */
  rev: number | bigint;
  /** Parent (created and deleted objects). */
  parentId?: string | null;
  /** Position in the parent's collection (created objects). */
  index?: number | bigint | null;
  /** Own-view RFC 6902 patch with `oldValue` (modified objects). */
  patch?: unknown[] | null;
}

/** `doc.diff` result. */
export interface doc_diff_result {
  /** Base revision. */
  fromRev: number | bigint;
  /** End revision. */
  toRev: number | bigint;
  /** Net changes. */
  changes: doc_diff_ChangeRecord[];
  /** More changes than `limit`. */
  truncated: boolean;
}

/** Export scope. */
export type doc_export_json_ExportScope = 'project' | 'scenes' | 'elements';

/** Binary handling. */
export type doc_export_json_Binaries = 'embed' | 'reference' | 'omit';

/** What to do when a target file exists (01 §11.2 rule 4). */
export type doc_export_json_OnExists = 'fail' | 'overwrite';

/** `doc.export.json` parameters. */
export interface doc_export_json_params {
  /** What to export. */
  scope: doc_export_json_ExportScope;
  /** Scenes or elements. */
  refs?: string[] | null;
  /** Target file (default: inline). */
  path?: string | null;
  /** Binary data. */
  binaries?: doc_export_json_Binaries;
  /** Indented. */
  pretty?: boolean;
  /** Overwrite policy. */
  onExists?: doc_export_json_OnExists;
}

/** `doc.export.json` result. */
export interface doc_export_json_result {
  /** Inline bundle. */
  bundle?: Record<string, unknown> | null;
  /** File written. */
  path?: string | null;
  /** Size. */
  bytes: number | bigint;
  /** Objects. */
  objectCount: number | bigint;
}

/** ID policy. */
export type doc_import_json_IdPolicy = 'new' | 'preserve';

/** Conflict policy. */
export type doc_import_json_OnConflict = 'rename' | 'replace' | 'fail';

/** Colour policy. */
export type doc_import_json_ColourPolicy = 'keepExisting' | 'updateExisting';

/** `doc.import.json` parameters. */
export interface doc_import_json_params {
  /** A path or an inline bundle. */
  source: unknown;
  /** Target scene (omit: new scenes). */
  target?: string | null;
  /** Scene position. */
  index?: number | null;
  /** First frame. */
  atFrame?: number;
  /** ID policy. */
  idPolicy?: doc_import_json_IdPolicy;
  /** Conflict policy. */
  onConflict?: doc_import_json_OnConflict;
  /** Colour policy. */
  colours?: doc_import_json_ColourPolicy;
  /** Report only. */
  dryRun?: boolean;
}

export type doc_import_json_result = Record<string, unknown>;

/** No parameters. */
export type doc_loadReport_get_params = Record<string, unknown>;

export type doc_loadReport_get_result = Record<string, unknown>;

/** Sort order of `doc.object.find`. */
export type doc_object_find_Sort = 'path' | 'name' | 'rev' | 'createdAt';

/** `doc.object.find` parameters. */
export interface doc_object_find_params {
  /** Object type (or alias). */
  type?: string | null;
  /** Only descendants of this object. */
  within?: string | null;
  /** Name: exact, or a glob with `*` and `?`. */
  name?: string | null;
  /** Predicates (10 §10.3). */
  where?: Record<string, unknown> | null;
  /** Page size. */
  limit?: number;
  /** First item. */
  offset?: number;
  /** Extra top-level properties per item. */
  fields?: string[] | null;
  /** Order. */
  sort?: doc_object_find_Sort;
}

/** `doc.object.find` result. */
export interface doc_object_find_result {
  /** Items `{id, type, name, path, rev, …fields}`. */
  items: (Record<string, unknown>)[];
  /** Matches in total. */
  total: number | bigint;
  /** Offset of the next page (`null` at the end). */
  nextOffset?: number | bigint | null;
}

/** What to add to returned objects. */
export type doc_object_get_Include = 'derived' | 'stats';

/** Paging of one large array inside the object (AG-05). */
export interface doc_object_get_Page {
  /** JSON Pointer of the array inside the object. */
  pointer: string;
  /** First item. */
  offset?: number;
  /** Items. */
  limit?: number;
}

/** `doc.object.get` parameters. */
export interface doc_object_get_params {
  /** One object (id, compound id, path or special reference). */
  ref?: string | null;
  /** Several objects (`@selection` expands to the selection). */
  refs?: string[] | null;
  /** Object levels in full: 1 = children as stubs, 0 = the object as a stub, -1 = everything (SI-011). */
  depth?: number;
  /** Top-level properties to keep (`id` and `type` are always kept). */
  fields?: string[] | null;
  /** Extra data. */
  include?: doc_object_get_Include[];
  /** Page through one large array. */
  page?: doc_object_get_Page | null;
}

/** Paging information. */
export interface doc_object_get_PageInfo {
  /** The paged array. */
  pointer: string;
  /** Items in the array. */
  total: number | bigint;
  /** Offset of the next page (`null` at the end). */
  nextOffset?: number | bigint | null;
}

/** `doc.object.get` result. */
export interface doc_object_get_result {
  /** The object (single `ref`). */
  object?: Record<string, unknown> | null;
  /** The objects (`refs`). */
  objects?: (Record<string, unknown>)[] | null;
  /** Statistics per object (`include: ["stats"]`). */
  stats?: unknown[] | null;
  /** Paging of the array named by `page.pointer`, per object. */
  page?: doc_object_get_PageInfo[] | null;
}

/** One RFC 6902 operation, pointers relative to the object. */
export interface doc_object_patch_OpParam {
  /** Operation. */
  op: doc_object_patch_OpKind;
  /** Target pointer. */
  path: string;
  /** Value (`add`, `replace`, `test`). */
  value?: unknown;
  /** Source pointer (`move`, `copy`). */
  from?: string | null;
}

/** RFC 6902 operation names. */
export type doc_object_patch_OpKind = 'add' | 'remove' | 'replace' | 'move' | 'copy' | 'test';

/** `expectRev`: one revision for every target, or per reference. */
export type doc_object_patch_ExpectRev = number | bigint | Record<string, number | bigint>;

/** `doc.object.patch` parameters. */
export interface doc_object_patch_params {
  /** Objects to patch (ids, paths, `@selection`). */
  targets: string[];
  /** JSON merge patch (RFC 7386) applied to every target. */
  props?: Record<string, unknown> | null;
  /** JSON Patch operations (RFC 6902), pointers relative to each target. */
  ops?: doc_object_patch_OpParam[] | null;
  /** Optimistic concurrency (REV-05). */
  expectRev?: doc_object_patch_ExpectRev | null;
  /** Undo label (default "Set <property>"). */
  label?: string | null;
  /** Report the changes without applying them (the executor reads it before the handler runs). */
  dryRun?: boolean;
}

/** One object change of a committed (or dry-run) transaction (10 §2.6, 11 §7.2). */
export interface doc_object_patch_ChangeRecord {
  /** Object id (compound for container-local ids). */
  id: string;
  /** Object type. */
  type: string;
  /** `created`, `modified` or `deleted`. */
  op: string;
  /** The object's `rev` after the transaction (the `documentRev` it would get for a dry run). */
  rev: number | bigint;
  /** Parent (created and deleted objects). */
  parentId?: string | null;
  /** Position in the parent's collection (created objects). */
  index?: number | bigint | null;
  /** Own-view RFC 6902 patch with `oldValue` (modified objects). */
  patch?: unknown[] | null;
}

/** `doc.object.patch` result. */
export interface doc_object_patch_result {
  /** The changes (also in the envelope). */
  changes: doc_object_patch_ChangeRecord[];
  /** `documentRev` after the patch (the revision it would get for a dry run). */
  documentRev: number | bigint;
}

/** Reference direction. */
export type doc_object_refs_RefDirection = 'incoming' | 'outgoing';

/** `doc.object.refs` parameters. */
export interface doc_object_refs_params {
  /** The object. */
  ref: string;
  /** Incoming or outgoing. */
  direction?: doc_object_refs_RefDirection;
  /** Maximum. */
  limit?: number;
}

/** `doc.object.refs` result. */
export interface doc_object_refs_result {
  /** `{id, type, path, property}`. */
  refs: (Record<string, unknown>)[];
  /** Total. */
  total: number | bigint;
}

/** `doc.object.resolve` parameters. */
export interface doc_object_resolve_params {
  /** References (ids, compound ids, paths, special references). */
  refs: string[];
}

/** `doc.object.resolve` result. */
export interface doc_object_resolve_result {
  /** One item per reference: `{ref, id, type, name, path, rev}` or `{ref, error}`. */
  items: (Record<string, unknown>)[];
}

/** No parameters. */
export type doc_package_changedOnDisk_params = Record<string, unknown>;

export type doc_package_changedOnDisk_result = Record<string, unknown>;

/** Package form. */
export type doc_package_export_Form = 'package' | 'folder';

/** What to do when a target file exists (01 §11.2 rule 4). */
export type doc_package_export_OnExists = 'fail' | 'overwrite';

/** `doc.package.export` parameters. */
export interface doc_package_export_params {
  /** Target path. */
  path: string;
  /** Form. */
  form?: doc_package_export_Form;
  /** Overwrite policy. */
  onExists?: doc_package_export_OnExists;
}

/** `{jobId}`. */
export interface doc_package_export_result {
  /** The job. */
  jobId: string;
}

/** `doc.package.get` parameters. */
export interface doc_package_get_params {
  /** Package path. */
  path: string;
  /** Validate fully. */
  validate?: boolean;
}

export type doc_package_get_result = Record<string, unknown>;

/** `doc.repair` parameters. */
export interface doc_repair_params {
  /** Issues to fix. */
  issueIds?: string[] | null;
  /** Issue codes to fix. */
  codes?: string[] | null;
  /** Subtree. */
  within?: string | null;
  /** Every fixable issue. */
  all?: boolean;
  /** Report only. */
  dryRun?: boolean;
}

/** One object change of a committed (or dry-run) transaction (10 §2.6, 11 §7.2). */
export interface doc_repair_ChangeRecord {
  /** Object id (compound for container-local ids). */
  id: string;
  /** Object type. */
  type: string;
  /** `created`, `modified` or `deleted`. */
  op: string;
  /** The object's `rev` after the transaction (the `documentRev` it would get for a dry run). */
  rev: number | bigint;
  /** Parent (created and deleted objects). */
  parentId?: string | null;
  /** Position in the parent's collection (created objects). */
  index?: number | bigint | null;
  /** Own-view RFC 6902 patch with `oldValue` (modified objects). */
  patch?: unknown[] | null;
}

/** `doc.repair` result. */
export interface doc_repair_result {
  /** Fixed issues. */
  fixed: string[];
  /** `{issueId, reason}`. */
  skipped: (Record<string, unknown>)[];
  /** Changes. */
  changes: doc_repair_ChangeRecord[];
}

/** `doc.schema.get` parameters. */
export interface doc_schema_get_params {
  /** Object type (or alias). */
  type: string;
  /** JSON Pointer of one property. */
  property?: string | null;
}

/** `doc.schema.get` result. */
export interface doc_schema_get_result {
  /** Type name. */
  type: string;
  /** Format version. */
  formatVersion: string;
  /** The JSON Schema (draft 2020-12). */
  schema: Record<string, unknown>;
}

/** No parameters. */
export type doc_schema_list_params = Record<string, unknown>;

/** One type in `doc.schema.list`. */
export interface doc_schema_list_TypeInfo {
  /** Type name. */
  type: string;
  /** Path aliases. */
  aliases: string[];
  /** Possible parent types. */
  parents: string[];
  /** Addressable by quoted name in paths. */
  nameAddressable: boolean;
  /** Description. */
  description: string;
}

/** `doc.schema.list` result. */
export interface doc_schema_list_result {
  /** Format version. */
  formatVersion: string;
  /** Types. */
  types: doc_schema_list_TypeInfo[];
}

/** Validation mode. */
export type doc_schema_validate_ValidateMode = 'create' | 'replace';

/** `doc.schema.validate` parameters. */
export interface doc_schema_validate_params {
  /** Object type. */
  type: string;
  /** Candidate object. */
  object: Record<string, unknown>;
  /** Create (id/rev may be absent) or replace. */
  mode?: doc_schema_validate_ValidateMode;
}

/** Validation result. */
export interface doc_schema_validate_result {
  /** No errors. */
  valid: boolean;
  /** `{pointer, code, message}`. */
  errors: (Record<string, unknown>)[];
}

/** No parameters. */
export type doc_state_get_params = Record<string, unknown>;

/** `doc.state.get` result. */
export interface doc_state_get_result {
  /** Project id. */
  projectId: string;
  /** Package path (`null` until saved). */
  path?: string | null;
  /** `package` or `folder` (`null` until saved). */
  form?: string | null;
  /** Format version. */
  formatVersion: string;
  /** Version migrated from on load (C1.3). */
  migratedFrom?: string | null;
  /** Current revision. */
  documentRev: number | bigint;
  /** Revision at the last save. */
  savedRev?: number | bigint | null;
  /** Modified since the last save. */
  dirty: boolean;
  /** Read-only. */
  readOnly: boolean;
  /** Lock holder (C1.3). */
  lock?: unknown;
  /** Oldest revision `doc.diff` accepts. */
  historyFromRev: number | bigint;
}

/** Severity filter. */
export type doc_validate_Severity = 'info' | 'warning' | 'error';

/** `doc.validate` parameters. */
export interface doc_validate_params {
  /** Subtree. */
  within?: string | null;
  /** Checks (default all). */
  checks?: string[] | null;
  /** Minimum severity. */
  minSeverity?: doc_validate_Severity;
  /** Maximum issues. */
  limit?: number;
}

/** `doc.validate` result. */
export interface doc_validate_result {
  /** Revision checked. */
  documentRev: number | bigint;
  /** Issues (10 §10.4 `Issue`). */
  issues: (Record<string, unknown>)[];
  /** Counts by severity. */
  counts: Record<string, number | bigint>;
  /** More issues than `limit`. */
  truncated: boolean;
}

/** No parameters. */
export type edit_clipboard_changed_params = Record<string, unknown>;

/** Clipboard content kinds (01 §12.4). */
export type edit_clipboard_changed_ClipboardKind = 'drawingObjects' | 'elements' | 'elementProperties' | 'cells' | 'keyframes' | 'motionPoint' | 'templates' | 'functionPoints' | 'image';

/** `{kind, itemCount}`. */
export interface edit_clipboard_changed_result {
  /** Kind. */
  kind: edit_clipboard_changed_ClipboardKind;
  /** Items. */
  itemCount: number;
}

/** `edit.clipboard.get` parameters. */
export interface edit_clipboard_get_params {
  /** Include the items. */
  includeData?: boolean;
}

/** `Clipboard` (01 §12.4). */
export interface edit_clipboard_get_Clipboard {
  /** Content kind. */
  kind: edit_clipboard_get_ClipboardKind;
  /** Context it was copied from. */
  sourceContext?: string | null;
  /** Project it was copied from. */
  sourceProjectId?: string | null;
  /** Number of items. */
  itemCount: number;
  /** Items in the section-10 shape of the kind (present only with `includeData`). */
  items?: (Record<string, unknown>)[] | null;
  /** Cut-special options. */
  options?: Record<string, unknown> | null;
}

/** Clipboard content kinds (01 §12.4). */
export type edit_clipboard_get_ClipboardKind = 'drawingObjects' | 'elements' | 'elementProperties' | 'cells' | 'keyframes' | 'motionPoint' | 'templates' | 'functionPoints' | 'image';

/** `edit.clipboard.get` result: the clipboard, or `null` when it is empty. */
export interface edit_clipboard_get_result {
  /** Content. */
  clipboard?: edit_clipboard_get_Clipboard | null;
}

/** `Clipboard` (01 §12.4). */
export interface edit_clipboard_set_Clipboard {
  /** Content kind. */
  kind: edit_clipboard_set_ClipboardKind;
  /** Context it was copied from. */
  sourceContext?: string | null;
  /** Project it was copied from. */
  sourceProjectId?: string | null;
  /** Number of items. */
  itemCount: number;
  /** Items in the section-10 shape of the kind (present only with `includeData`). */
  items?: (Record<string, unknown>)[] | null;
  /** Cut-special options. */
  options?: Record<string, unknown> | null;
}

/** Clipboard content kinds (01 §12.4). */
export type edit_clipboard_set_ClipboardKind = 'drawingObjects' | 'elements' | 'elementProperties' | 'cells' | 'keyframes' | 'motionPoint' | 'templates' | 'functionPoints' | 'image';

/** `edit.clipboard.set` parameters. */
export interface edit_clipboard_set_params {
  /** The content. */
  content: edit_clipboard_set_Clipboard;
}

/** `{kind, itemCount}`. */
export interface edit_clipboard_set_result {
  /** Kind. */
  kind: edit_clipboard_set_ClipboardKind;
  /** Items. */
  itemCount: number;
}

/** Selection / clipboard context (01 §6.2); `auto` follows the focused panel. */
export type edit_copy_EditContext = 'auto' | 'drawing' | 'staging' | 'xsheet' | 'timeline' | 'library' | 'functionEditor';

/** `context` + `objectIds` parameters of cut, copy and delete. */
export interface edit_copy_params {
  /** Context; `auto` follows the focused panel. */
  context?: edit_copy_EditContext;
  /** Objects (default: the selection). */
  objectIds?: string[] | null;
}

/** `{kind, itemCount}`. */
export interface edit_copy_ClipboardSummary {
  /** Kind. */
  kind: edit_copy_ClipboardKind;
  /** Items. */
  itemCount: number;
}

/** Clipboard content kinds (01 §12.4). */
export type edit_copy_ClipboardKind = 'drawingObjects' | 'elements' | 'elementProperties' | 'cells' | 'keyframes' | 'motionPoint' | 'templates' | 'functionPoints' | 'image';

/** `{clipboard: {kind, itemCount}}`. */
export interface edit_copy_result {
  /** What the clipboard holds now. */
  clipboard: edit_copy_ClipboardSummary;
}

/** Selection / clipboard context (01 §6.2); `auto` follows the focused panel. */
export type edit_cut_EditContext = 'auto' | 'drawing' | 'staging' | 'xsheet' | 'timeline' | 'library' | 'functionEditor';

/** `context` + `objectIds` parameters of cut, copy and delete. */
export interface edit_cut_params {
  /** Context; `auto` follows the focused panel. */
  context?: edit_cut_EditContext;
  /** Objects (default: the selection). */
  objectIds?: string[] | null;
}

/** `{kind, itemCount}`. */
export interface edit_cut_ClipboardSummary {
  /** Kind. */
  kind: edit_cut_ClipboardKind;
  /** Items. */
  itemCount: number;
}

/** Clipboard content kinds (01 §12.4). */
export type edit_cut_ClipboardKind = 'drawingObjects' | 'elements' | 'elementProperties' | 'cells' | 'keyframes' | 'motionPoint' | 'templates' | 'functionPoints' | 'image';

/** `{clipboard: {kind, itemCount}}`. */
export interface edit_cut_result {
  /** What the clipboard holds now. */
  clipboard: edit_cut_ClipboardSummary;
}

/** `edit.cutSpecial` parameters. */
export interface edit_cutSpecial_params {
  /** Objects (default: the selection). */
  objectIds?: string[] | null;
  /** `TimelineEditOptions` (01 §12.4); omitted fields take the remembered values. */
  options?: Record<string, unknown> | null;
  /** Remember the options. */
  remember?: boolean;
}

/** `{kind, itemCount}`. */
export interface edit_cutSpecial_ClipboardSummary {
  /** Kind. */
  kind: edit_cutSpecial_ClipboardKind;
  /** Items. */
  itemCount: number;
}

/** Clipboard content kinds (01 §12.4). */
export type edit_cutSpecial_ClipboardKind = 'drawingObjects' | 'elements' | 'elementProperties' | 'cells' | 'keyframes' | 'motionPoint' | 'templates' | 'functionPoints' | 'image';

/** `{clipboard: {kind, itemCount}}`. */
export interface edit_cutSpecial_result {
  /** What the clipboard holds now. */
  clipboard: edit_cutSpecial_ClipboardSummary;
}

/** Selection / clipboard context (01 §6.2); `auto` follows the focused panel. */
export type edit_delete_EditContext = 'auto' | 'drawing' | 'staging' | 'xsheet' | 'timeline' | 'library' | 'functionEditor';

/** `context` + `objectIds` parameters of cut, copy and delete. */
export interface edit_delete_params {
  /** Context; `auto` follows the focused panel. */
  context?: edit_delete_EditContext;
  /** Objects (default: the selection). */
  objectIds?: string[] | null;
}

/** `{deletedIds}`. */
export interface edit_delete_result {
  /** Deleted objects. */
  deletedIds: string[];
}

/** Selection / clipboard context (01 §6.2); `auto` follows the focused panel. */
export type edit_deselectAll_EditContext = 'auto' | 'drawing' | 'staging' | 'xsheet' | 'timeline' | 'library' | 'functionEditor';

/** `{context}` parameters. */
export interface edit_deselectAll_params {
  /** Selection context; `auto` follows the focused panel. */
  context?: edit_deselectAll_EditContext;
}

/** `{context}`. */
export interface edit_deselectAll_result {
  /** The resolved context. */
  context: string;
}

/** No parameters. */
export type edit_history_changed_params = Record<string, unknown>;

/** `edit.history.changed` payload. */
export interface edit_history_changed_result {
  /** Position. */
  position: number | bigint;
  /** Entries. */
  entryCount: number | bigint;
  /** Label of the top step. */
  lastLabel?: string | null;
  /** `user` or `agent`. */
  source?: string | null;
}

/** `edit.history.get` parameters. */
export interface edit_history_get_params {
  /** Entries to skip (from the oldest). */
  offset?: number;
  /** Entries to return. */
  limit?: number;
}

/** One history entry (01 §12.1). */
export interface edit_history_get_HistoryEntry {
  /** 1-based position of the step (position `n` = steps 1…n applied). */
  index: number | bigint;
  /** Step id (`u-<n>`), as in envelopes' `undo.stepId`. */
  stepId: string;
  /** Label. */
  label: string;
  /** `user` or `agent`. */
  source: string;
  /** Agent session id. */
  agentSessionId?: string | null;
  /** Agent display name. */
  agentName?: string | null;
  /** Commit time (ISO 8601 UTC). */
  time: string;
  /** Changed objects. */
  objectCount: number | bigint;
}

/** `edit.history.get` result. */
export interface edit_history_get_result {
  /** Entries, oldest first. */
  entries: edit_history_get_HistoryEntry[];
  /** Steps applied. */
  position: number | bigint;
  /** Position of the last save (`null` if unreachable). */
  cleanPosition?: number | bigint | null;
  /** Total entries. */
  total: number | bigint;
}

/** `edit.history.move` parameters. */
export interface edit_history_move_params {
  /** Target position (0 = before the first step, entry count = after the last). */
  position: number;
}

/** `{position}`. */
export interface edit_history_move_result {
  /** History position after the call. */
  position: number | bigint;
}

/** Selection / clipboard context (01 §6.2); `auto` follows the focused panel. */
export type edit_paste_EditContext = 'auto' | 'drawing' | 'staging' | 'xsheet' | 'timeline' | 'library' | 'functionEditor';

/** `edit.paste` parameters. */
export interface edit_paste_params {
  /** Context; `auto` follows the focused panel. */
  context?: edit_paste_EditContext;
  /** Where to paste: `{drawingId}`, `{elementId, frame}`, `{catalogId}`, `{functionId, frame}` or `{objectIds}`. */
  destination?: Record<string, unknown> | null;
}

/** A warning (11 §6.3, §10): same code space as errors, never aborts. */
export interface edit_paste_Warning {
  /** Code. */
  code: string;
  /** Message. */
  message: string;
  /** Affected objects. */
  objectIds?: string[];
}

/** Paste result. */
export interface edit_paste_result {
  /** Created objects. */
  createdIds: string[];
  /** Modified objects. */
  modifiedIds: string[];
  /** Warnings. */
  warnings: edit_paste_Warning[];
}

/** `edit.pasteAsNewElement` parameters. */
export interface edit_pasteAsNewElement_params {
  /** Scene (default current). */
  sceneId?: string | null;
  /** Frame (default current). */
  frame?: number | null;
}

/** `{elementId}`. */
export interface edit_pasteAsNewElement_result {
  /** The new element. */
  elementId: string;
}

/** `edit.pasteSpecial` parameters. */
export interface edit_pasteSpecial_params {
  /** Where to paste. */
  destination?: Record<string, unknown> | null;
  /** `TimelineEditOptions` or `MotionPointPasteOptions` (01 §12.4). */
  options?: Record<string, unknown> | null;
  /** Use the last options. */
  useLast?: boolean;
  /** Remember the options. */
  remember?: boolean;
}

/** A warning (11 §6.3, §10): same code space as errors, never aborts. */
export interface edit_pasteSpecial_Warning {
  /** Code. */
  code: string;
  /** Message. */
  message: string;
  /** Affected objects. */
  objectIds?: string[];
}

/** Paste result. */
export interface edit_pasteSpecial_result {
  /** Created objects. */
  createdIds: string[];
  /** Modified objects. */
  modifiedIds: string[];
  /** Warnings. */
  warnings: edit_pasteSpecial_Warning[];
}

/** `edit.redo` parameters. */
export interface edit_redo_params {
  /** Steps to redo. */
  steps?: number;
}

/** `edit.redo` result. */
export interface edit_redo_result {
  /** Labels of the redone steps, in order. */
  redone: string[];
  /** History position after the call. */
  position: number | bigint;
}

/** Selection / clipboard context (01 §6.2); `auto` follows the focused panel. */
export type edit_selectAll_EditContext = 'auto' | 'drawing' | 'staging' | 'xsheet' | 'timeline' | 'library' | 'functionEditor';

/** `{context}` parameters. */
export interface edit_selectAll_params {
  /** Selection context; `auto` follows the focused panel. */
  context?: edit_selectAll_EditContext;
}

/** `edit.selectAll` result. */
export interface edit_selectAll_result {
  /** The resolved context. */
  context: string;
  /** The selection. */
  ids: string[];
}

/** No parameters. */
export type edit_selection_changed_params = Record<string, unknown>;

/** `{context, ids}`. */
export interface edit_selection_changed_result {
  /** The resolved context. */
  context: string;
  /** The selection. */
  ids: string[];
}

/** Selection / clipboard context (01 §6.2); `auto` follows the focused panel. */
export type edit_selection_get_EditContext = 'auto' | 'drawing' | 'staging' | 'xsheet' | 'timeline' | 'library' | 'functionEditor';

/** `{context}` parameters. */
export interface edit_selection_get_params {
  /** Selection context; `auto` follows the focused panel. */
  context?: edit_selection_get_EditContext;
}

/** `edit.selection.get` result. */
export interface edit_selection_get_result {
  /** The resolved context. */
  context: string;
  /** Selected ids. */
  ids: string[];
  /** Object types present. */
  kinds: string[];
}

/** Selection / clipboard context (01 §6.2); `auto` follows the focused panel. */
export type edit_selection_set_EditContext = 'auto' | 'drawing' | 'staging' | 'xsheet' | 'timeline' | 'library' | 'functionEditor';

/** Selection change mode. */
export type edit_selection_set_SelectMode = 'replace' | 'add' | 'remove' | 'toggle';

/** `edit.selection.set` parameters. */
export interface edit_selection_set_params {
  /** Selection context. */
  context?: edit_selection_set_EditContext;
  /** Objects (ids or paths). */
  ids: string[];
  /** How to combine with the current selection. */
  mode?: edit_selection_set_SelectMode;
}

/** `{context, ids}`. */
export interface edit_selection_set_result {
  /** The resolved context. */
  context: string;
  /** The selection. */
  ids: string[];
}

/** `edit.undo` parameters. */
export interface edit_undo_params {
  /** Steps to undo (each is its own transaction). */
  steps?: number;
  /** Undo only if this step id is still the most recent one (`undo.notTop` otherwise; 11 §7.7). */
  ifTop?: string | null;
}

/** `edit.undo` result. */
export interface edit_undo_result {
  /** Labels of the undone steps, most recent first. */
  undone: string[];
  /** History position after the call. */
  position: number | bigint;
}

/** `inspect.contactSheet.render` parameters. */
export interface inspect_contactSheet_render_params {
  /** Scene. */
  sceneId?: string | null;
  /** Frames: a list, or `{from, to, step}`. */
  frames: unknown;
  /** Columns. */
  columns?: number | null;
  /** Cell width. */
  cellWidth?: number | null;
  /** Frame labels. */
  labels?: boolean | null;
}

export type inspect_contactSheet_render_result = Record<string, unknown>;

/** A point with an optional depth. */
export interface inspect_coordinates_convert_Point3 {
  /** X. */
  x: number;
  /** Y. */
  y: number;
  /** Z. */
  z?: number | null;
}

/** `inspect.coordinates.convert` parameters. */
export interface inspect_coordinates_convert_params {
  /** Source space. */
  from: string;
  /** Target space. */
  to: string;
  /** Points. */
  points: inspect_coordinates_convert_Point3[];
  /** Scene. */
  sceneId?: string | null;
  /** Frame. */
  frame?: number | null;
  /** Element. */
  elementId?: string | null;
  /** View. */
  view?: string | null;
}

/** `{points}`. */
export interface inspect_coordinates_convert_result {
  /** Converted points. */
  points: inspect_coordinates_convert_Point3[];
}

/** Description detail. */
export type inspect_frame_describe_Detail = 'summary' | 'full';

/** `inspect.frame.describe` parameters. */
export interface inspect_frame_describe_params {
  /** Scene (default current). */
  sceneId?: string | null;
  /** Frame (default current). */
  frame?: number | null;
  /** Camera (default the scene's current camera). */
  cameraId?: string | null;
  /** Detail. */
  detail?: inspect_frame_describe_Detail;
}

export type inspect_frame_describe_result = Record<string, unknown>;

/** Coordinate space. */
export type inspect_hitTest_Space = 'view' | 'drawing' | 'scene' | 'camera' | 'image';

/** A 2-D point. */
export interface inspect_hitTest_Point {
  /** X. */
  x: number;
  /** Y. */
  y: number;
}

/** `inspect.hitTest` parameters. */
export interface inspect_hitTest_params {
  /** Space of `point`. */
  space: inspect_hitTest_Space;
  /** The point. */
  point: inspect_hitTest_Point;
  /** Scene. */
  sceneId?: string | null;
  /** Frame. */
  frame?: number | null;
  /** View. */
  view?: string | null;
}

/** `{hits}`. */
export interface inspect_hitTest_result {
  /** Hits, front to back. */
  hits: (Record<string, unknown>)[];
}

/** A rectangle. */
export interface inspect_screenshot_Rect {
  /** Left. */
  x: number;
  /** Top. */
  y: number;
  /** Width. */
  w: number;
  /** Height. */
  h: number;
}

/** `inspect.screenshot` parameters. */
export interface inspect_screenshot_params {
  /** Window, view, panel or dialog. */
  target?: string | null;
  /** Region. */
  region?: inspect_screenshot_Rect | null;
}

export type inspect_screenshot_result = Record<string, unknown>;

/** `inspect.ui.tree` parameters. */
export interface inspect_ui_tree_params {
  /** Window, panel or dialog. */
  target?: string | null;
  /** Depth. */
  depth?: number | null;
}

export type inspect_ui_tree_result = Record<string, unknown>;

/** `{jobId}` parameters. */
export interface job_cancel_params {
  /** The job. */
  jobId: string;
}

/** Job state (11 §7.7). */
export type job_cancel_JobState = 'queued' | 'running' | 'succeeded' | 'failed' | 'cancelled';

/** A typed command error in the `{code, message, details, hint?}` wire form (11 §10). */
export interface job_cancel_CommandError {
  /** Stable dotted code `area.reason`. */
  code: string;
  /** Human-readable message. */
  message: string;
  /** Structured details (boxed: the error travels in every `Result` of the command path, so it stays small). */
  details: unknown;
  /** How to fix the call, where known. */
  hint?: string | null;
}

/** A job record (11 §7.7 `job_status`). */
export interface job_cancel_result {
  /** Job id. */
  jobId: string;
  /** The job command. */
  commandId: string;
  /** State. */
  state: job_cancel_JobState;
  /** Progress 0..1. */
  progress: number;
  /** Current step text. */
  message: string;
  /** Start time (ISO 8601 UTC). */
  startedAt: string;
  /** Final result. */
  result?: unknown;
  /** Final error. */
  error?: job_cancel_CommandError | null;
  /** Session that started it. */
  session: string;
}

/** No parameters. */
export type job_finished_params = Record<string, unknown>;

/** Job state (11 §7.7). */
export type job_finished_JobState = 'queued' | 'running' | 'succeeded' | 'failed' | 'cancelled';

/** A typed command error in the `{code, message, details, hint?}` wire form (11 §10). */
export interface job_finished_CommandError {
  /** Stable dotted code `area.reason`. */
  code: string;
  /** Human-readable message. */
  message: string;
  /** Structured details (boxed: the error travels in every `Result` of the command path, so it stays small). */
  details: unknown;
  /** How to fix the call, where known. */
  hint?: string | null;
}

/** A job record (11 §7.7 `job_status`). */
export interface job_finished_result {
  /** Job id. */
  jobId: string;
  /** The job command. */
  commandId: string;
  /** State. */
  state: job_finished_JobState;
  /** Progress 0..1. */
  progress: number;
  /** Current step text. */
  message: string;
  /** Start time (ISO 8601 UTC). */
  startedAt: string;
  /** Final result. */
  result?: unknown;
  /** Final error. */
  error?: job_finished_CommandError | null;
  /** Session that started it. */
  session: string;
}

/** `job.list` parameters. */
export interface job_list_params {
  /** One job. */
  jobId?: string | null;
  /** Every session's jobs (default: the caller's own). */
  all?: boolean;
}

/** A job record (11 §7.7 `job_status`). */
export interface job_list_JobRecord {
  /** Job id. */
  jobId: string;
  /** The job command. */
  commandId: string;
  /** State. */
  state: job_list_JobState;
  /** Progress 0..1. */
  progress: number;
  /** Current step text. */
  message: string;
  /** Start time (ISO 8601 UTC). */
  startedAt: string;
  /** Final result. */
  result?: unknown;
  /** Final error. */
  error?: job_list_CommandError | null;
  /** Session that started it. */
  session: string;
}

/** Job state (11 §7.7). */
export type job_list_JobState = 'queued' | 'running' | 'succeeded' | 'failed' | 'cancelled';

/** A typed command error in the `{code, message, details, hint?}` wire form (11 §10). */
export interface job_list_CommandError {
  /** Stable dotted code `area.reason`. */
  code: string;
  /** Human-readable message. */
  message: string;
  /** Structured details (boxed: the error travels in every `Result` of the command path, so it stays small). */
  details: unknown;
  /** How to fix the call, where known. */
  hint?: string | null;
}

export type job_list_result = job_list_JobRecord[];

/** No parameters. */
export type job_progress_params = Record<string, unknown>;

/** Job state (11 §7.7). */
export type job_progress_JobState = 'queued' | 'running' | 'succeeded' | 'failed' | 'cancelled';

/** A typed command error in the `{code, message, details, hint?}` wire form (11 §10). */
export interface job_progress_CommandError {
  /** Stable dotted code `area.reason`. */
  code: string;
  /** Human-readable message. */
  message: string;
  /** Structured details (boxed: the error travels in every `Result` of the command path, so it stays small). */
  details: unknown;
  /** How to fix the call, where known. */
  hint?: string | null;
}

/** A job record (11 §7.7 `job_status`). */
export interface job_progress_result {
  /** Job id. */
  jobId: string;
  /** The job command. */
  commandId: string;
  /** State. */
  state: job_progress_JobState;
  /** Progress 0..1. */
  progress: number;
  /** Current step text. */
  message: string;
  /** Start time (ISO 8601 UTC). */
  startedAt: string;
  /** Final result. */
  result?: unknown;
  /** Final error. */
  error?: job_progress_CommandError | null;
  /** Session that started it. */
  session: string;
}

/** `job.wait` parameters. */
export interface job_wait_params {
  /** The job. */
  jobId: string;
  /** How long to wait. */
  timeoutMs?: number;
}

/** Job state (11 §7.7). */
export type job_wait_JobState = 'queued' | 'running' | 'succeeded' | 'failed' | 'cancelled';

/** A typed command error in the `{code, message, details, hint?}` wire form (11 §10). */
export interface job_wait_CommandError {
  /** Stable dotted code `area.reason`. */
  code: string;
  /** Human-readable message. */
  message: string;
  /** Structured details (boxed: the error travels in every `Result` of the command path, so it stays small). */
  details: unknown;
  /** How to fix the call, where known. */
  hint?: string | null;
}

/** `job.wait` result: the job record and whether the wait timed out. */
export interface job_wait_result {
  /** Job id. */
  jobId: string;
  /** The job command. */
  commandId: string;
  /** State. */
  state: job_wait_JobState;
  /** Progress 0..1. */
  progress: number;
  /** Current step text. */
  message: string;
  /** Start time (ISO 8601 UTC). */
  startedAt: string;
  /** Final result. */
  result?: unknown;
  /** Final error. */
  error?: job_wait_CommandError | null;
  /** Session that started it. */
  session: string;
  /** The job was still running when the timeout passed. */
  timedOut: boolean;
}

/** No parameters. */
export type project_autosaved_params = Record<string, unknown>;

export type project_autosaved_result = Record<string, unknown>;

/** `project.backup.list` parameters. */
export interface project_backup_list_params {
  /** Project path (default: the current project). */
  path?: string | null;
}

export type project_backup_list_result = (Record<string, unknown>)[];

/** What to do with unsaved changes (01 §11.2 rule 3). */
export type project_backup_open_OnUnsaved = 'fail' | 'save' | 'discard';

/** `project.backup.open` parameters. */
export interface project_backup_open_params {
  /** Backup id. */
  backupId: string;
  /** Unsaved-changes policy. */
  onUnsaved?: project_backup_open_OnUnsaved;
}

/** `{projectId}`. */
export interface project_backup_open_result {
  /** Project id. */
  projectId: string;
}

/** What to do with unsaved changes (01 §11.2 rule 3). */
export type project_close_OnUnsaved = 'fail' | 'save' | 'discard';

/** `{onUnsaved}` parameters. */
export interface project_close_params {
  /** Unsaved-changes policy. */
  onUnsaved?: project_close_OnUnsaved;
}

/** `{closed}`. */
export interface project_close_result {
  /** The project was closed. */
  closed: boolean;
}

/** No parameters. */
export type project_closed_params = Record<string, unknown>;

/** `{projectId}`. */
export interface project_closed_result {
  /** Project id. */
  projectId: string;
}

/** What to do with unsaved changes (01 §11.2 rule 3). */
export type project_create_OnUnsaved = 'fail' | 'save' | 'discard';

/** What to do when a target file exists (01 §11.2 rule 4). */
export type project_create_OnExists = 'fail' | 'overwrite';

/** `project.create` parameters. */
export interface project_create_params {
  /** Project name. */
  name: string;
  /** Save immediately at this absolute path. */
  path?: string | null;
  /** Format preset id. */
  preset?: string | null;
  /** Frame rate. */
  frameRate?: number | null;
  /** Camera width. */
  cameraWidth?: number | null;
  /** Camera height. */
  cameraHeight?: number | null;
  /** Unsaved-changes policy. */
  onUnsaved?: project_create_OnUnsaved;
  /** Overwrite policy. */
  onExists?: project_create_OnExists;
}

/** `project.create` result. */
export interface project_create_result {
  /** Project id. */
  projectId: string;
  /** Saved path. */
  path?: string | null;
  /** First scene. */
  sceneId: string;
  /** Its camera. */
  cameraId: string;
  /** Its drawing element. */
  elementId: string;
}

/** No parameters. */
export type project_dirty_changed_params = Record<string, unknown>;

/** `project.dirty.changed` payload. */
export interface project_dirty_changed_result {
  /** Project modified. */
  projectDirty: boolean;
  /** Global library modified. */
  globalLibraryDirty: boolean;
}

/** No parameters. */
export type project_info_get_params = Record<string, unknown>;

export type project_info_get_result = Record<string, unknown>;

/** What to do with unsaved changes (01 §11.2 rule 3). */
export type project_open_OnUnsaved = 'fail' | 'save' | 'discard';

/** `project.open` parameters. */
export interface project_open_params {
  /** Absolute package path. */
  path: string;
  /** Open read-only. */
  readOnly?: boolean;
  /** Unsaved-changes policy. */
  onUnsaved?: project_open_OnUnsaved;
}

/** `{jobId}`. */
export interface project_open_result {
  /** The job. */
  jobId: string;
}

/** No parameters. */
export type project_opened_params = Record<string, unknown>;

/** `project.opened` payload. */
export interface project_opened_result {
  /** Project id. */
  projectId: string;
  /** Path. */
  path?: string | null;
  /** Read-only. */
  readOnly: boolean;
}

/** No parameters. */
export type project_presets_list_params = Record<string, unknown>;

export type project_presets_list_result = (Record<string, unknown>)[];

/** No parameters. */
export type project_properties_get_params = Record<string, unknown>;

/** Project properties. */
export interface project_properties_get_result {
  /** Frame rate. */
  frameRate: number;
  /** Camera width. */
  cameraWidth: number;
  /** Camera height. */
  cameraHeight: number;
}

/** `project.properties.set` parameters. */
export interface project_properties_set_params {
  /** Frame rate. */
  frameRate?: number | null;
  /** Camera width. */
  cameraWidth?: number | null;
  /** Camera height. */
  cameraHeight?: number | null;
  /** Make these the defaults for new projects. */
  setAsDefault?: boolean;
}

/** Project properties. */
export interface project_properties_set_result {
  /** Frame rate. */
  frameRate: number;
  /** Camera width. */
  cameraWidth: number;
  /** Camera height. */
  cameraHeight: number;
}

/** No parameters. */
export type project_recent_list_params = Record<string, unknown>;

export type project_recent_list_result = (Record<string, unknown>)[];

/** `project.recent.remove` parameters. */
export interface project_recent_remove_params {
  /** One entry. */
  path?: string | null;
  /** Every entry. */
  all?: boolean;
}

/** `{remaining}`. */
export interface project_recent_remove_result {
  /** Entries left. */
  remaining: number;
}

/** `project.recovery.delete` parameters. */
export interface project_recovery_delete_params {
  /** One entry. */
  recoveryId?: string | null;
  /** Every entry. */
  all?: boolean;
}

/** `{deleted}`. */
export interface project_recovery_delete_result {
  /** Entries deleted. */
  deleted: number;
}

/** No parameters. */
export type project_recovery_list_params = Record<string, unknown>;

export type project_recovery_list_result = (Record<string, unknown>)[];

/** What to do with unsaved changes (01 §11.2 rule 3). */
export type project_recovery_open_OnUnsaved = 'fail' | 'save' | 'discard';

/** `project.recovery.open` parameters. */
export interface project_recovery_open_params {
  /** Recovery id. */
  recoveryId: string;
  /** Unsaved-changes policy. */
  onUnsaved?: project_recovery_open_OnUnsaved;
}

/** `{jobId}`. */
export interface project_recovery_open_result {
  /** The job. */
  jobId: string;
}

/** `project.save` parameters. */
export interface project_save_params {
  /** Also save the global library. */
  includeGlobalLibrary?: boolean;
  /** Allow writing a newer format version. */
  allowFormatUpgrade?: boolean;
}

/** `{jobId}`. */
export interface project_save_result {
  /** The job. */
  jobId: string;
}

/** What to do when a target file exists (01 §11.2 rule 4). */
export type project_saveAs_OnExists = 'fail' | 'overwrite';

/** `project.saveAs` parameters. */
export interface project_saveAs_params {
  /** Absolute target path. */
  path: string;
  /** Overwrite policy. */
  onExists?: project_saveAs_OnExists;
  /** Also save the global library. */
  includeGlobalLibrary?: boolean;
  /** Allow writing a newer format version. */
  allowFormatUpgrade?: boolean;
}

/** `{jobId}`. */
export interface project_saveAs_result {
  /** The job. */
  jobId: string;
}

/** No parameters. */
export type project_saved_params = Record<string, unknown>;

export type project_saved_result = Record<string, unknown>;

/** `{visible}` parameters and result. */
export interface window_floatingPanels_set_params {
  /** Show (true) or hide (false). */
  visible: boolean;
}

/** `{visible}` parameters and result. */
export interface window_floatingPanels_set_result {
  /** Show (true) or hide (false). */
  visible: boolean;
}

/** `{enabled}` parameters and result. */
export interface window_fullScreen_set_params {
  /** The state to set (explicit, never a toggle; readiness bullet 4). */
  enabled: boolean;
}

/** `{enabled}` parameters and result. */
export interface window_fullScreen_set_result {
  /** The state to set (explicit, never a toggle; readiness bullet 4). */
  enabled: boolean;
}

/** No parameters. */
export type window_geometry_get_params = Record<string, unknown>;

/** A screen. */
export interface window_geometry_get_Screen {
  /** Left. */
  x: number;
  /** Top. */
  y: number;
  /** Width. */
  width: number;
  /** Height. */
  height: number;
  /** Scale factor. */
  scale: number;
}

/** Window geometry. */
export interface window_geometry_get_result {
  /** Left. */
  x: number;
  /** Top. */
  y: number;
  /** Width. */
  width: number;
  /** Height. */
  height: number;
  /** Maximised. */
  maximized: boolean;
  /** Full screen. */
  fullScreen: boolean;
  /** Screens. */
  screens: window_geometry_get_Screen[];
}

/** `window.geometry.set` parameters. */
export interface window_geometry_set_params {
  /** Left. */
  x?: number | null;
  /** Top. */
  y?: number | null;
  /** Width. */
  width?: number | null;
  /** Height. */
  height?: number | null;
  /** Maximised. */
  maximized?: boolean | null;
}

/** A screen. */
export interface window_geometry_set_Screen {
  /** Left. */
  x: number;
  /** Top. */
  y: number;
  /** Width. */
  width: number;
  /** Height. */
  height: number;
  /** Scale factor. */
  scale: number;
}

/** Window geometry. */
export interface window_geometry_set_result {
  /** Left. */
  x: number;
  /** Top. */
  y: number;
  /** Width. */
  width: number;
  /** Height. */
  height: number;
  /** Maximised. */
  maximized: boolean;
  /** Full screen. */
  fullScreen: boolean;
  /** Screens. */
  screens: window_geometry_set_Screen[];
}

/** `{minimized}`. */
export interface window_minimized_set_params {
  /** Minimised. */
  minimized: boolean;
}

/** `{minimized}`. */
export interface window_minimized_set_result {
  /** Minimised. */
  minimized: boolean;
}

/** No parameters. */
export type window_panel_list_params = Record<string, unknown>;

/** A panel as listed by `window.panel.list` (01 A). */
export interface window_panel_list_PanelEntry {
  /** Panel id. */
  panelId: string;
  /** Title. */
  title: string;
  /** Shown. */
  visible: boolean;
  /** Floating. */
  floating: boolean;
  /** Dock area (`left` … `bottom`, `central`). */
  area?: string | null;
  /** Index of its tab group within the area. */
  tabGroup?: number | bigint | null;
  /** In front of its tab group. */
  current: boolean;
  /** Floating geometry. */
  geometry?: window_panel_list_Geometry | null;
  /** Has focus. */
  focused: boolean;
}

/** A geometry in logical pixels. */
export interface window_panel_list_Geometry {
  /** Left. */
  x: number;
  /** Top. */
  y: number;
  /** Width. */
  width: number;
  /** Height. */
  height: number;
}

export type window_panel_list_result = window_panel_list_PanelEntry[];

/** Dock area. */
export type window_panel_move_Area = 'left' | 'right' | 'top' | 'bottom' | 'central';

/** Placement relative to `relativeTo`. */
export type window_panel_move_Placement = 'tab' | 'before' | 'after' | 'above' | 'below';

/** `window.panel.move` parameters. */
export interface window_panel_move_params {
  /** Panel id. */
  panelId: string;
  /** Target area. */
  area: window_panel_move_Area;
  /** A panel docked in the area. */
  relativeTo?: string | null;
  /** Placement. */
  position?: window_panel_move_Placement;
  /** Area width or height. */
  size?: number | null;
}

/** Corner ownership (01 §5.4 *Extend side panels*). */
export type window_panel_move_Corners = 'sides' | 'topBottom';

/** The four dock areas, each an ordered stack of tab groups. */
export interface window_panel_move_Docks {
  /** Left area. */
  left?: window_panel_move_TabGroup[];
  /** Right area. */
  right?: window_panel_move_TabGroup[];
  /** Top area. */
  top?: window_panel_move_TabGroup[];
  /** Bottom area. */
  bottom?: window_panel_move_TabGroup[];
}

/** One tab group of a dock area. */
export interface window_panel_move_TabGroup {
  /** Panel ids, in tab order. */
  tabs: string[];
  /** The tab in front. */
  current: string;
  /** On the first group of an area: the area's width (left/right) or height (top/bottom). */
  size?: number | null;
}

/** A floating panel. */
export interface window_panel_move_Floating {
  /** Panel id. */
  panelId: string;
  /** Shown. */
  visible: boolean;
  /** Window geometry. */
  geometry: window_panel_move_Geometry;
}

/** A geometry in logical pixels. */
export interface window_panel_move_Geometry {
  /** Left. */
  x: number;
  /** Top. */
  y: number;
  /** Width. */
  width: number;
  /** Height. */
  height: number;
}

/** A tool palette. */
export interface window_panel_move_ToolPalette {
  /** Palette id (`drawingTools`, …). */
  id: string;
  /** Shown. */
  visible: boolean;
  /** Floating (else docked with the main toolbar). */
  floating: boolean;
  /** Floating position. */
  position?: window_panel_move_Position | null;
}

/** A point in logical pixels. */
export interface window_panel_move_Position {
  /** Left. */
  x: number;
  /** Top. */
  y: number;
}

/** The main toolbar configuration (01 §5.7). */
export interface window_panel_move_MainToolbar {
  /** Shown. */
  visible: boolean;
  /** Items: command references, group ids, `separator`, `space`, `flexibleSpace`. */
  items: string[];
}

/** An arrangement of the main window (01 §12.4 `Layout`, without the nested full-screen arrangement). */
export interface window_panel_move_Arrangement {
  /** The central view. */
  centralPanel: string;
  /** Corner ownership. */
  corners: window_panel_move_Corners;
  /** Dock areas. */
  docks: window_panel_move_Docks;
  /** Floating panels. */
  floating?: window_panel_move_Floating[];
  /** Tool palettes. */
  toolPalettes?: window_panel_move_ToolPalette[];
  /** Main toolbar. */
  mainToolbar: window_panel_move_MainToolbar;
}

/** A workspace layout on the wire (01 §12.4 `Layout`): the normal arrangement's fields plus the separate full-screen arrangement (01 §5.6). Panels not mentioned are hidden. */
export interface window_panel_move_result {
  /** The central view. */
  centralPanel: string;
  /** Corner ownership. */
  corners: window_panel_move_Corners;
  /** Dock areas. */
  docks: window_panel_move_Docks;
  /** Floating panels. */
  floating?: window_panel_move_Floating[];
  /** Tool palettes. */
  toolPalettes?: window_panel_move_ToolPalette[];
  /** Main toolbar. */
  mainToolbar: window_panel_move_MainToolbar;
  /** Full-screen arrangement (same shape without `fullScreen`). */
  fullScreen: window_panel_move_Arrangement;
}

/** A geometry in logical pixels. */
export interface window_panel_set_Geometry {
  /** Left. */
  x: number;
  /** Top. */
  y: number;
  /** Width. */
  width: number;
  /** Height. */
  height: number;
}

/** `window.panel.set` parameters. */
export interface window_panel_set_params {
  /** Panel id (01 §5.2) or `active` for the focused panel. */
  panelId: string;
  /** Show or hide. */
  visible?: boolean | null;
  /** Float or dock. */
  floating?: boolean | null;
  /** Give it focus. */
  focus?: boolean;
  /** Floating geometry. */
  geometry?: window_panel_set_Geometry | null;
}

/** A panel as listed by `window.panel.list` (01 A). */
export interface window_panel_set_result {
  /** Panel id. */
  panelId: string;
  /** Title. */
  title: string;
  /** Shown. */
  visible: boolean;
  /** Floating. */
  floating: boolean;
  /** Dock area (`left` … `bottom`, `central`). */
  area?: string | null;
  /** Index of its tab group within the area. */
  tabGroup?: number | bigint | null;
  /** In front of its tab group. */
  current: boolean;
  /** Floating geometry. */
  geometry?: window_panel_set_Geometry | null;
  /** Has focus. */
  focused: boolean;
}

/** No parameters. */
export type window_raiseAll_run_params = Record<string, unknown>;

/** An empty result object. */
export type window_raiseAll_run_result = Record<string, unknown>;

/** `{enabled}` parameters and result. */
export interface window_sidePanelsExtended_set_params {
  /** The state to set (explicit, never a toggle; readiness bullet 4). */
  enabled: boolean;
}

/** `{enabled}` parameters and result. */
export interface window_sidePanelsExtended_set_result {
  /** The state to set (explicit, never a toggle; readiness bullet 4). */
  enabled: boolean;
}

/** Only the main toolbar is configurable. */
export type window_toolbar_items_reset_MainOnly = 'main';

/** `window.toolbar.items.reset` parameters. */
export interface window_toolbar_items_reset_params {
  /** The main toolbar. */
  toolbarId?: window_toolbar_items_reset_MainOnly;
}

/** `{items}`. */
export interface window_toolbar_items_reset_result {
  /** The toolbar's items. */
  items: string[];
}

/** Only the main toolbar is configurable. */
export type window_toolbar_items_set_MainOnly = 'main';

/** `window.toolbar.items.set` parameters. */
export interface window_toolbar_items_set_params {
  /** The main toolbar. */
  toolbarId?: window_toolbar_items_set_MainOnly;
  /** Items: command references, group ids, `separator`, `space`, `flexibleSpace`. */
  items: string[];
}

/** `{items}`. */
export interface window_toolbar_items_set_result {
  /** The toolbar's items. */
  items: string[];
}

/** No parameters. */
export type window_toolbar_list_params = Record<string, unknown>;

/** A toolbar as listed (01 §5.7). */
export interface window_toolbar_list_ToolbarEntry {
  /** `main` or a tool palette id. */
  toolbarId: string;
  /** Shown. */
  visible: boolean;
  /** Floating. */
  floating: boolean;
  /** Floating position. */
  position?: window_toolbar_list_Position | null;
  /** Items (main toolbar). */
  items: string[];
  /** Items that may be placed (main toolbar). */
  allowedItems: string[];
}

/** A point in logical pixels. */
export interface window_toolbar_list_Position {
  /** Left. */
  x: number;
  /** Top. */
  y: number;
}

export type window_toolbar_list_result = window_toolbar_list_ToolbarEntry[];

/** Toolbar ids. */
export type window_toolbar_set_ToolbarId = 'main' | 'drawingTools' | 'animationTools' | 'boneTools' | 'effectTools' | 'soundTools';

/** A point in logical pixels. */
export interface window_toolbar_set_Position {
  /** Left. */
  x: number;
  /** Top. */
  y: number;
}

/** `window.toolbar.set` parameters. */
export interface window_toolbar_set_params {
  /** The toolbar. */
  toolbarId: window_toolbar_set_ToolbarId;
  /** Shown. */
  visible: boolean;
  /** Floating (tool palettes). */
  floating?: boolean | null;
  /** Floating position. */
  position?: window_toolbar_set_Position | null;
}

/** A toolbar as listed (01 §5.7). */
export interface window_toolbar_set_result {
  /** `main` or a tool palette id. */
  toolbarId: string;
  /** Shown. */
  visible: boolean;
  /** Floating. */
  floating: boolean;
  /** Floating position. */
  position?: window_toolbar_set_Position | null;
  /** Items (main toolbar). */
  items: string[];
  /** Items that may be placed (main toolbar). */
  allowedItems: string[];
}

/** No parameters. */
export type workspace_changed_params = Record<string, unknown>;

/** `workspace.changed` payload. */
export interface workspace_changed_result {
  /** Current workspace. */
  name: string;
  /** The arrangement differs from the stored one. */
  modified: boolean;
}

/** A workspace layout on the wire (01 §12.4 `Layout`): the normal arrangement's fields plus the separate full-screen arrangement (01 §5.6). Panels not mentioned are hidden. */
export interface workspace_create_Layout {
  /** The central view. */
  centralPanel: string;
  /** Corner ownership. */
  corners: workspace_create_Corners;
  /** Dock areas. */
  docks: workspace_create_Docks;
  /** Floating panels. */
  floating?: workspace_create_Floating[];
  /** Tool palettes. */
  toolPalettes?: workspace_create_ToolPalette[];
  /** Main toolbar. */
  mainToolbar: workspace_create_MainToolbar;
  /** Full-screen arrangement (same shape without `fullScreen`). */
  fullScreen: workspace_create_Arrangement;
}

/** Corner ownership (01 §5.4 *Extend side panels*). */
export type workspace_create_Corners = 'sides' | 'topBottom';

/** The four dock areas, each an ordered stack of tab groups. */
export interface workspace_create_Docks {
  /** Left area. */
  left?: workspace_create_TabGroup[];
  /** Right area. */
  right?: workspace_create_TabGroup[];
  /** Top area. */
  top?: workspace_create_TabGroup[];
  /** Bottom area. */
  bottom?: workspace_create_TabGroup[];
}

/** One tab group of a dock area. */
export interface workspace_create_TabGroup {
  /** Panel ids, in tab order. */
  tabs: string[];
  /** The tab in front. */
  current: string;
  /** On the first group of an area: the area's width (left/right) or height (top/bottom). */
  size?: number | null;
}

/** A floating panel. */
export interface workspace_create_Floating {
  /** Panel id. */
  panelId: string;
  /** Shown. */
  visible: boolean;
  /** Window geometry. */
  geometry: workspace_create_Geometry;
}

/** A geometry in logical pixels. */
export interface workspace_create_Geometry {
  /** Left. */
  x: number;
  /** Top. */
  y: number;
  /** Width. */
  width: number;
  /** Height. */
  height: number;
}

/** A tool palette. */
export interface workspace_create_ToolPalette {
  /** Palette id (`drawingTools`, …). */
  id: string;
  /** Shown. */
  visible: boolean;
  /** Floating (else docked with the main toolbar). */
  floating: boolean;
  /** Floating position. */
  position?: workspace_create_Position | null;
}

/** A point in logical pixels. */
export interface workspace_create_Position {
  /** Left. */
  x: number;
  /** Top. */
  y: number;
}

/** The main toolbar configuration (01 §5.7). */
export interface workspace_create_MainToolbar {
  /** Shown. */
  visible: boolean;
  /** Items: command references, group ids, `separator`, `space`, `flexibleSpace`. */
  items: string[];
}

/** An arrangement of the main window (01 §12.4 `Layout`, without the nested full-screen arrangement). */
export interface workspace_create_Arrangement {
  /** The central view. */
  centralPanel: string;
  /** Corner ownership. */
  corners: workspace_create_Corners;
  /** Dock areas. */
  docks: workspace_create_Docks;
  /** Floating panels. */
  floating?: workspace_create_Floating[];
  /** Tool palettes. */
  toolPalettes?: workspace_create_ToolPalette[];
  /** Main toolbar. */
  mainToolbar: workspace_create_MainToolbar;
}

/** `workspace.create` parameters. */
export interface workspace_create_params {
  /** New workspace name. */
  name: string;
  /** 1-based position in the list (default: last). */
  position?: number | null;
  /** Layout (default: the current arrangement). */
  layout?: workspace_create_Layout | null;
}

/** `{name, position}`. */
export interface workspace_create_result {
  /** Name. */
  name: string;
  /** Position. */
  position: number | bigint;
}

/** `{name}` parameters. */
export interface workspace_delete_params {
  /** Workspace name. */
  name: string;
}

/** `{deleted}`. */
export interface workspace_delete_result {
  /** Deleted workspace. */
  deleted: string;
}

/** What to do when a target file exists (01 §11.2 rule 4). */
export type workspace_export_OnExists = 'fail' | 'overwrite';

/** `workspace.export` parameters. */
export interface workspace_export_params {
  /** Workspace name. */
  name: string;
  /** Absolute target path. */
  path: string;
  /** Overwrite policy. */
  onExists?: workspace_export_OnExists;
}

/** `{path}`. */
export interface workspace_export_result {
  /** The file written. */
  path: string;
}

/** `{name?}` parameters. */
export interface workspace_get_params {
  /** Workspace name (default: the current arrangement). */
  name?: string | null;
}

/** Corner ownership (01 §5.4 *Extend side panels*). */
export type workspace_get_Corners = 'sides' | 'topBottom';

/** The four dock areas, each an ordered stack of tab groups. */
export interface workspace_get_Docks {
  /** Left area. */
  left?: workspace_get_TabGroup[];
  /** Right area. */
  right?: workspace_get_TabGroup[];
  /** Top area. */
  top?: workspace_get_TabGroup[];
  /** Bottom area. */
  bottom?: workspace_get_TabGroup[];
}

/** One tab group of a dock area. */
export interface workspace_get_TabGroup {
  /** Panel ids, in tab order. */
  tabs: string[];
  /** The tab in front. */
  current: string;
  /** On the first group of an area: the area's width (left/right) or height (top/bottom). */
  size?: number | null;
}

/** A floating panel. */
export interface workspace_get_Floating {
  /** Panel id. */
  panelId: string;
  /** Shown. */
  visible: boolean;
  /** Window geometry. */
  geometry: workspace_get_Geometry;
}

/** A geometry in logical pixels. */
export interface workspace_get_Geometry {
  /** Left. */
  x: number;
  /** Top. */
  y: number;
  /** Width. */
  width: number;
  /** Height. */
  height: number;
}

/** A tool palette. */
export interface workspace_get_ToolPalette {
  /** Palette id (`drawingTools`, …). */
  id: string;
  /** Shown. */
  visible: boolean;
  /** Floating (else docked with the main toolbar). */
  floating: boolean;
  /** Floating position. */
  position?: workspace_get_Position | null;
}

/** A point in logical pixels. */
export interface workspace_get_Position {
  /** Left. */
  x: number;
  /** Top. */
  y: number;
}

/** The main toolbar configuration (01 §5.7). */
export interface workspace_get_MainToolbar {
  /** Shown. */
  visible: boolean;
  /** Items: command references, group ids, `separator`, `space`, `flexibleSpace`. */
  items: string[];
}

/** An arrangement of the main window (01 §12.4 `Layout`, without the nested full-screen arrangement). */
export interface workspace_get_Arrangement {
  /** The central view. */
  centralPanel: string;
  /** Corner ownership. */
  corners: workspace_get_Corners;
  /** Dock areas. */
  docks: workspace_get_Docks;
  /** Floating panels. */
  floating?: workspace_get_Floating[];
  /** Tool palettes. */
  toolPalettes?: workspace_get_ToolPalette[];
  /** Main toolbar. */
  mainToolbar: workspace_get_MainToolbar;
}

/** A workspace layout on the wire (01 §12.4 `Layout`): the normal arrangement's fields plus the separate full-screen arrangement (01 §5.6). Panels not mentioned are hidden. */
export interface workspace_get_result {
  /** The central view. */
  centralPanel: string;
  /** Corner ownership. */
  corners: workspace_get_Corners;
  /** Dock areas. */
  docks: workspace_get_Docks;
  /** Floating panels. */
  floating?: workspace_get_Floating[];
  /** Tool palettes. */
  toolPalettes?: workspace_get_ToolPalette[];
  /** Main toolbar. */
  mainToolbar: workspace_get_MainToolbar;
  /** Full-screen arrangement (same shape without `fullScreen`). */
  fullScreen: workspace_get_Arrangement;
}

/** Import conflict policy. */
export type workspace_import_ImportOnExists = 'fail' | 'overwrite' | 'rename';

/** `workspace.import` parameters. */
export interface workspace_import_params {
  /** Absolute source path. */
  path: string;
  /** Name-conflict policy. */
  onExists?: workspace_import_ImportOnExists;
}

/** `{name}`. */
export interface workspace_import_result {
  /** Workspace name. */
  name: string;
}

/** A workspace layout on the wire (01 §12.4 `Layout`): the normal arrangement's fields plus the separate full-screen arrangement (01 §5.6). Panels not mentioned are hidden. */
export interface workspace_layout_set_Layout {
  /** The central view. */
  centralPanel: string;
  /** Corner ownership. */
  corners: workspace_layout_set_Corners;
  /** Dock areas. */
  docks: workspace_layout_set_Docks;
  /** Floating panels. */
  floating?: workspace_layout_set_Floating[];
  /** Tool palettes. */
  toolPalettes?: workspace_layout_set_ToolPalette[];
  /** Main toolbar. */
  mainToolbar: workspace_layout_set_MainToolbar;
  /** Full-screen arrangement (same shape without `fullScreen`). */
  fullScreen: workspace_layout_set_Arrangement;
}

/** Corner ownership (01 §5.4 *Extend side panels*). */
export type workspace_layout_set_Corners = 'sides' | 'topBottom';

/** The four dock areas, each an ordered stack of tab groups. */
export interface workspace_layout_set_Docks {
  /** Left area. */
  left?: workspace_layout_set_TabGroup[];
  /** Right area. */
  right?: workspace_layout_set_TabGroup[];
  /** Top area. */
  top?: workspace_layout_set_TabGroup[];
  /** Bottom area. */
  bottom?: workspace_layout_set_TabGroup[];
}

/** One tab group of a dock area. */
export interface workspace_layout_set_TabGroup {
  /** Panel ids, in tab order. */
  tabs: string[];
  /** The tab in front. */
  current: string;
  /** On the first group of an area: the area's width (left/right) or height (top/bottom). */
  size?: number | null;
}

/** A floating panel. */
export interface workspace_layout_set_Floating {
  /** Panel id. */
  panelId: string;
  /** Shown. */
  visible: boolean;
  /** Window geometry. */
  geometry: workspace_layout_set_Geometry;
}

/** A geometry in logical pixels. */
export interface workspace_layout_set_Geometry {
  /** Left. */
  x: number;
  /** Top. */
  y: number;
  /** Width. */
  width: number;
  /** Height. */
  height: number;
}

/** A tool palette. */
export interface workspace_layout_set_ToolPalette {
  /** Palette id (`drawingTools`, …). */
  id: string;
  /** Shown. */
  visible: boolean;
  /** Floating (else docked with the main toolbar). */
  floating: boolean;
  /** Floating position. */
  position?: workspace_layout_set_Position | null;
}

/** A point in logical pixels. */
export interface workspace_layout_set_Position {
  /** Left. */
  x: number;
  /** Top. */
  y: number;
}

/** The main toolbar configuration (01 §5.7). */
export interface workspace_layout_set_MainToolbar {
  /** Shown. */
  visible: boolean;
  /** Items: command references, group ids, `separator`, `space`, `flexibleSpace`. */
  items: string[];
}

/** An arrangement of the main window (01 §12.4 `Layout`, without the nested full-screen arrangement). */
export interface workspace_layout_set_Arrangement {
  /** The central view. */
  centralPanel: string;
  /** Corner ownership. */
  corners: workspace_layout_set_Corners;
  /** Dock areas. */
  docks: workspace_layout_set_Docks;
  /** Floating panels. */
  floating?: workspace_layout_set_Floating[];
  /** Tool palettes. */
  toolPalettes?: workspace_layout_set_ToolPalette[];
  /** Main toolbar. */
  mainToolbar: workspace_layout_set_MainToolbar;
}

/** `{layout}` parameters. */
export interface workspace_layout_set_params {
  /** The layout to apply (not saved). */
  layout: workspace_layout_set_Layout;
}

/** A workspace layout on the wire (01 §12.4 `Layout`): the normal arrangement's fields plus the separate full-screen arrangement (01 §5.6). Panels not mentioned are hidden. */
export interface workspace_layout_set_result {
  /** The central view. */
  centralPanel: string;
  /** Corner ownership. */
  corners: workspace_layout_set_Corners;
  /** Dock areas. */
  docks: workspace_layout_set_Docks;
  /** Floating panels. */
  floating?: workspace_layout_set_Floating[];
  /** Tool palettes. */
  toolPalettes?: workspace_layout_set_ToolPalette[];
  /** Main toolbar. */
  mainToolbar: workspace_layout_set_MainToolbar;
  /** Full-screen arrangement (same shape without `fullScreen`). */
  fullScreen: workspace_layout_set_Arrangement;
}

/** No parameters. */
export type workspace_list_params = Record<string, unknown>;

/** A workspace as listed by `workspace.list`. */
export interface workspace_list_WorkspaceEntry {
  /** Name. */
  name: string;
  /** Built in (cannot be deleted). */
  builtIn: boolean;
  /** The current arrangement differs from the stored one (current workspace only). */
  modified: boolean;
  /** Position in the list (1-based). */
  position: number | bigint;
  /** The current workspace. */
  current: boolean;
}

export type workspace_list_result = workspace_list_WorkspaceEntry[];

/** `workspace.rename` parameters. */
export interface workspace_rename_params {
  /** Current name. */
  name: string;
  /** New name. */
  newName: string;
}

/** `{name}`. */
export interface workspace_rename_result {
  /** Workspace name. */
  name: string;
}

/** `workspace.reset` parameters. */
export interface workspace_reset_params {
  /** Built-in workspace (default: current). */
  name?: string | null;
  /** Every built-in workspace. */
  all?: boolean;
  /** Select it afterwards. */
  select?: boolean;
}

/** `{reset}`. */
export interface workspace_reset_result {
  /** Reset workspaces. */
  reset: string[];
}

/** `{name?}` parameters. */
export interface workspace_save_params {
  /** Workspace name (default: the current arrangement). */
  name?: string | null;
}

/** `{name}`. */
export interface workspace_save_result {
  /** Workspace name. */
  name: string;
}

/** `workspace.select` parameters. */
export interface workspace_select_params {
  /** Workspace name. */
  name: string;
  /** Save the current arrangement into the current workspace first (default: `general.workspace.autoSave`, which is off until preferences arrive in M4). */
  saveCurrent?: boolean | null;
}

/** `{name}`. */
export interface workspace_select_result {
  /** Workspace name. */
  name: string;
}
