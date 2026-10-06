// SPDX-License-Identifier: Apache-2.0
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { App } from "../src/app";
import type { AppInfo } from "../src/ipc/bindings";
import { violations } from "../../ui/test/axe";

const info: AppInfo = { version: "0.1.0", platform: "macos", locale: "en" };

afterEach(() => clearMocks());

describe("App", () => {
  it("shows the about screen in the core's language and reports ready", async () => {
    const calls: string[] = [];
    mockIPC((cmd) => {
      calls.push(cmd);
      if (cmd === "app_info") return { ...info, locale: "zh-Hans" };
    });
    const { container } = render(<App />);
    expect(await screen.findByRole("heading", { level: 1, name: "关于 Scoplen" })).toBeTruthy();
    expect(screen.getByText("版本 0.1.0，适用于 macOS")).toBeTruthy();
    expect(document.documentElement.lang).toBe("zh-Hans");
    expect(calls).toContain("shell_ready");
    expect(await violations(container)).toEqual([]);
  });

  it("explains a failure to reach the core and retries", async () => {
    let attempts = 0;
    mockIPC((cmd) => {
      if (cmd === "app_info") {
        attempts += 1;
        if (attempts === 1) throw new Error("ipc unavailable");
        return info;
      }
    });
    const { container } = render(<App />);
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("Nothing was changed.");
    expect(alert.textContent).toContain("ipc unavailable");
    expect(await violations(container)).toEqual([]);
    await userEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByRole("heading", { level: 1, name: "About Scoplen" })).toBeTruthy();
  });
});
