// SPDX-License-Identifier: Apache-2.0
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Button, Lockup, Mark, SkipLink, Toolbar, markPath } from "../src";
import { violations } from "./axe";

describe("Button", () => {
  it("is a button that does not submit forms unless asked", () => {
    render(<Button>Add host</Button>);
    expect(screen.getByRole("button", { name: "Add host" })).toHaveProperty("type", "button");
  });

  it("keeps its label and size while busy and reports it", () => {
    render(<Button busy>Connect</Button>);
    const button = screen.getByRole("button", { name: "Connect" });
    expect(button.getAttribute("aria-busy")).toBe("true");
    expect(button).toHaveProperty("disabled", true);
    expect(button.textContent).toBe("Connect");
  });

  it("does not run its action while busy or disabled", async () => {
    const onClick = vi.fn();
    render(
      <>
        <Button busy onClick={onClick}>
          One
        </Button>
        <Button disabled onClick={onClick}>
          Two
        </Button>
      </>,
    );
    await userEvent.click(screen.getByRole("button", { name: "One" }));
    await userEvent.click(screen.getByRole("button", { name: "Two" }));
    expect(onClick).not.toHaveBeenCalled();
  });
});

describe("Toolbar", () => {
  function Bar() {
    return (
      <Toolbar label="Session">
        <Button>Copy</Button>
        <Button disabled>Paste</Button>
        <Button>Find</Button>
        <Button>Split</Button>
      </Toolbar>
    );
  }

  it("is one Tab stop", async () => {
    render(
      <>
        <Bar />
        <Button>After</Button>
      </>,
    );
    await userEvent.tab();
    expect(document.activeElement?.textContent).toBe("Copy");
    await userEvent.tab();
    expect(document.activeElement?.textContent).toBe("After");
  });

  it("moves with arrows, Home, and End, skipping disabled controls and wrapping", async () => {
    render(<Bar />);
    await userEvent.tab();
    await userEvent.keyboard("{ArrowRight}");
    expect(document.activeElement?.textContent).toBe("Find");
    await userEvent.keyboard("{End}");
    expect(document.activeElement?.textContent).toBe("Split");
    await userEvent.keyboard("{ArrowRight}");
    expect(document.activeElement?.textContent).toBe("Copy");
    await userEvent.keyboard("{ArrowLeft}");
    expect(document.activeElement?.textContent).toBe("Split");
    await userEvent.keyboard("{Home}");
    expect(document.activeElement?.textContent).toBe("Copy");
  });

  it("returns Tab to the control that last had focus", async () => {
    render(
      <>
        <Button>Before</Button>
        <Bar />
      </>,
    );
    await userEvent.tab();
    await userEvent.tab();
    await userEvent.keyboard("{ArrowRight}");
    await userEvent.tab({ shift: true });
    expect(document.activeElement?.textContent).toBe("Before");
    await userEvent.tab();
    expect(document.activeElement?.textContent).toBe("Find");
  });

  it("is named and oriented for assistive technology", () => {
    render(<Bar />);
    const toolbar = screen.getByRole("toolbar", { name: "Session" });
    expect(toolbar.getAttribute("aria-orientation")).toBe("horizontal");
  });
});

describe("Mark", () => {
  it("is decorative unless named", () => {
    const { container } = render(<Mark />);
    expect(container.querySelector("svg")?.getAttribute("aria-hidden")).toBe("true");
    render(<Mark title="Scoplen" />);
    expect(screen.getByRole("img", { name: "Scoplen" })).toBeTruthy();
  });

  it("follows the geometry of the visual identity", () => {
    expect(markPath("display")).toBe(
      "M11 8H34V15.5H17V34H8V11A3 3 0 0 1 11 8ZM53 56H30V48.5H47V30H56V53A3 3 0 0 1 53 56Z",
    );
    const { container } = render(<Mark variant="display" />);
    expect(container.querySelector("svg")?.getAttribute("viewBox")).toBe("8 8 48 48");
  });
});

describe("accessibility", () => {
  it("has no axe violations in a composed screen", async () => {
    const { container } = render(
      <div>
        <SkipLink target="main">Skip to content</SkipLink>
        <header>
          <Lockup />
        </header>
        <main id="main">
          <h1>Hosts</h1>
          <Toolbar label="Actions">
            <Button variant="primary">Add host</Button>
            <Button variant="ghost">Import</Button>
          </Toolbar>
        </main>
      </div>,
    );
    expect(await violations(container)).toEqual([]);
  });
});
