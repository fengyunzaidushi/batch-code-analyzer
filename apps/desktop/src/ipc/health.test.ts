import { afterEach, describe, expect, it, vi } from "vitest";
import type { HealthCheckResponse } from "@batch-code-analyzer/ipc-types";

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

import { checkBackendHealth } from "./health";

describe("checkBackendHealth", () => {
  afterEach(() => {
    invokeMock.mockReset();
  });

  it("returns the generated health check DTO", async () => {
    const response = readyResponse();
    invokeMock.mockResolvedValue(response);

    await expect(checkBackendHealth()).resolves.toEqual(response);
    expect(invokeMock).toHaveBeenCalledWith("health_check");
  });

  it("preserves an IPC rejection for the presentation layer", async () => {
    invokeMock.mockRejectedValue(new Error("IPC unavailable"));

    await expect(checkBackendHealth()).rejects.toThrow("IPC unavailable");
  });
});

function readyResponse(): HealthCheckResponse {
  return {
    schemaVersion: 1,
    status: "ready",
    appVersion: "0.1.0",
    databaseStatus: "ready",
    databaseSchemaVersion: 1,
  };
}
