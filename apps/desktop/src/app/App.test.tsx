import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

const { checkBackendHealthMock } = vi.hoisted(() => ({
  checkBackendHealthMock: vi.fn(),
}));

vi.mock("../ipc/health", () => ({
  checkBackendHealth: checkBackendHealthMock,
}));

import { App } from "./App";

describe("App", () => {
  afterEach(() => {
    checkBackendHealthMock.mockReset();
  });

  it("shows a checking state while the command is pending", () => {
    checkBackendHealthMock.mockReturnValue(new Promise(() => undefined));

    render(<App />);

    expect(screen.getByText("正在检查")).toBeInTheDocument();
  });

  it("shows the ready state after a successful health check", async () => {
    checkBackendHealthMock.mockResolvedValue(undefined);

    render(<App />);

    expect(await screen.findByText("本地核心已就绪")).toBeInTheDocument();
  });

  it("shows a safe error and can retry", async () => {
    const user = userEvent.setup();
    checkBackendHealthMock
      .mockRejectedValueOnce(new Error("sensitive backend detail"))
      .mockResolvedValueOnce(undefined);

    render(<App />);

    expect(await screen.findByText("无法连接本地核心")).toBeInTheDocument();
    expect(
      screen.queryByText("sensitive backend detail"),
    ).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "重新检查" }));

    expect(await screen.findByText("本地核心已就绪")).toBeInTheDocument();
    expect(checkBackendHealthMock).toHaveBeenCalledTimes(2);
  });
});
