import { afterEach, describe, expect, it, vi } from "vitest";

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

  it("accepts the expected health response", async () => {
    invokeMock.mockResolvedValue("ok");

    await expect(checkBackendHealth()).resolves.toBeUndefined();
    expect(invokeMock).toHaveBeenCalledWith("health_check");
  });

  it("rejects an unexpected response", async () => {
    invokeMock.mockResolvedValue(null);

    await expect(checkBackendHealth()).rejects.toThrow(
      "Unexpected health check response",
    );
  });

  it("preserves an IPC rejection for the presentation layer", async () => {
    invokeMock.mockRejectedValue(new Error("IPC unavailable"));

    await expect(checkBackendHealth()).rejects.toThrow("IPC unavailable");
  });
});
