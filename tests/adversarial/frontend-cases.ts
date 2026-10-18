import { createHostApi } from "../../desktop/src/ipc/hostApi";
import {
  createMockTransport,
  MOCK_ENGINE_INFO,
} from "../../desktop/src/ipc/mockTransport";
import type { HostTransport } from "../../desktop/src/ipc/transport";
import { runIpcBench } from "../../desktop/src/diagnostics/ipcBench";

type TestApi = Pick<
  typeof import("../../desktop/node_modules/vitest/dist/index.js"),
  "it" | "expect"
>;

/** Registered by the desktop test bridge so no extra workspace/dependency is needed. */
export function registerAdversarialTests({ it, expect }: TestApi): void {
  function responding(value: unknown): HostTransport {
    return { ...createMockTransport(), invoke: () => Promise.resolve(value) };
  }

  for (const field of ["platform", "arch", "runMode"]) {
    it(`C01-R1-03: rejects missing EngineInfo.${field}`, async () => {
      const value: Record<string, unknown> = { ...MOCK_ENGINE_INFO };
      delete value[field];
      await expect(
        createHostApi(responding(value)).appInfo(),
      ).rejects.toMatchObject({ code: "ipc.invalidResponse" });
    });
  }
  it("C01-R1-03: rejects an unknown run mode", async () => {
    await expect(
      createHostApi(
        responding({ ...MOCK_ENGINE_INFO, runMode: "telepathy" }),
      ).appInfo(),
    ).rejects.toMatchObject({ code: "ipc.invalidResponse" });
  });
  it("C01-R1-03: rejects a negative Rust u64 ping field", async () => {
    await expect(
      createHostApi(
        responding({ seq: -1, payload: null, handlerNs: 0, engineUptimeNs: 0 }),
      ).ping({ seq: 1 }),
    ).rejects.toMatchObject({ code: "ipc.invalidResponse" });
  });
  it("C01-R1-03: rejects array-shaped menu parameters", async () => {
    const transport = createMockTransport();
    const seen: unknown[] = [];
    await createHostApi(transport).onMenuCommand((event) => seen.push(event));
    transport.emit("menu-command", {
      command: "workspace.reset",
      params: [],
      handler: "frontend",
      milestone: null,
    });
    expect(seen).toEqual([]);
  });
  it("C01-R1-05: mock enforces the same UTF-8 byte limit as Rust", async () => {
    const payload = "\u00e9".repeat(524_289); // 1,048,578 UTF-8 bytes, but only 524,289 JS code units.
    const outcome: unknown = await createHostApi(createMockTransport())
      .ping({ seq: 1, payload })
      .then(
        () => "accepted",
        (error: unknown) => error,
      );
    expect(outcome).toMatchObject({
      code: "limit.exceeded",
      details: { value: 1_048_578, limit: 1_048_576 },
    });
  });
  it("C01-R1-07: IPC benchmark fails if the return payload is dropped", async () => {
    const transport = createMockTransport();
    const host = createHostApi({
      ...transport,
      invoke: async (command, args) => {
        const value: unknown = await transport.invoke(command, args);
        if (command === "ping" && typeof value === "object" && value !== null)
          return { ...value, payload: null };
        return value;
      },
    });
    const report = await runIpcBench(host, { count: 2, payloadBytes: [1024] });
    expect(report.ok).toBe(false);
  });
}
