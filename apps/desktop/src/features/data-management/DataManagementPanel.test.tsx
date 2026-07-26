import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { DataManagementPanel } from "./DataManagementPanel";

describe("DataManagementPanel", () => {
  it("requires a second confirmation before scheduling a reset", async () => {
    const user = userEvent.setup();
    const reset = vi.fn().mockResolvedValue(undefined);
    const close = vi.fn();
    render(
      <DataManagementPanel active onClose={close} onResetAppData={reset} />,
    );

    await user.click(screen.getByRole("button", { name: "清空本地数据" }));
    expect(reset).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "确认删除" }));

    expect(reset).toHaveBeenCalledOnce();
    expect(close).not.toHaveBeenCalled();
    expect(screen.getByText(/仓库目录/)).toBeInTheDocument();
  });

  it("keeps the application reset available without a selected project", () => {
    render(
      <DataManagementPanel
        active
        onClose={() => undefined}
        onResetAppData={async () => undefined}
      />,
    );

    expect(screen.getByRole("button", { name: "清空本地数据" })).toBeEnabled();
  });
});
