import { render, screen } from "@solidjs/testing-library";
import { afterEach, describe, expect, it, vi } from "vitest";

import { SourceBlock } from "./SourceBlock.jsx";

describe("SourceBlock", () => {
  afterEach(() => {
    vi.restoreAllMocks();
    vi.useRealTimers();
  });

  it("shows the declared language and the verbatim code", () => {
    const { container } = render(() => (
      <SourceBlock lang="rust" code={'fn main() {\n    println!("hi");\n}'} />
    ));
    expect(container.querySelector(".org-src__lang")?.textContent).toBe("rust");
    expect(container.querySelector(".org-src__code")?.textContent).toBe(
      'fn main() {\n    println!("hi");\n}',
    );
  });

  it("omits a language tag when the block declares none", () => {
    const { container } = render(() => <SourceBlock lang={null} code="x = 1" />);
    expect(container.querySelector(".org-src__lang")?.textContent).toBe("");
  });

  it("copies the code to the clipboard and confirms, then reverts", async () => {
    vi.useFakeTimers();
    const writeText = vi.fn(() => Promise.resolve());
    vi.stubGlobal("navigator", { clipboard: { writeText } });

    render(() => <SourceBlock lang="python" code="print(1)" />);
    const button = screen.getByRole("button", { name: "Copy code to clipboard" });
    expect(button.textContent).toBe("Copy");

    button.click();
    await vi.waitFor(() => expect(writeText).toHaveBeenCalledWith("print(1)"));
    expect(button.textContent).toBe("Copied");

    await vi.advanceTimersByTimeAsync(1200);
    expect(button.textContent).toBe("Copy");

    vi.unstubAllGlobals();
  });

  it("reports a refused copy and selects the code by hand instead", async () => {
    vi.useFakeTimers();
    const writeText = vi.fn(() => Promise.reject(new Error("denied")));
    vi.stubGlobal("navigator", { clipboard: { writeText } });

    const { container } = render(() => <SourceBlock lang="sh" code="ls -la" />);
    const button = screen.getByRole("button", { name: "Copy code to clipboard" });

    button.click();
    await vi.waitFor(() => expect(button.textContent).toBe("Copy blocked"));
    expect(button).toHaveAccessibleName(
      "The clipboard refused the copy; the code is selected to copy by hand",
    );
    expect(window.getSelection()?.toString()).toBe("ls -la");

    await vi.advanceTimersByTimeAsync(1200);
    expect(button.textContent).toBe("Copy");
    expect(container.querySelector(".org-src__copy--failed")).toBeNull();

    vi.unstubAllGlobals();
  });

  // No clipboard API at all is the same refusal, reached before any request.
  it("reports a browser that exposes no clipboard", async () => {
    vi.useFakeTimers();
    vi.stubGlobal("navigator", {});

    render(() => <SourceBlock lang={null} code="x = 1" />);
    const button = screen.getByRole("button", { name: "Copy code to clipboard" });

    button.click();
    await vi.waitFor(() => expect(button.textContent).toBe("Copy blocked"));

    vi.unstubAllGlobals();
  });

  it("drops a copy that succeeds after the block was removed", async () => {
    vi.useFakeTimers();
    const scheduled = vi.spyOn(window, "setTimeout");
    let settle: () => void = () => {};
    const writeText = vi.fn(
      () =>
        new Promise<void>((resolve) => {
          settle = resolve;
        }),
    );
    vi.stubGlobal("navigator", { clipboard: { writeText } });

    const { container, unmount } = render(() => (
      <SourceBlock lang="rust" code="fn f() {}" />
    ));
    screen.getByRole("button", { name: "Copy code to clipboard" }).click();
    expect(writeText).toHaveBeenCalledTimes(1);

    unmount();
    const timers = scheduled.mock.calls.length;
    settle();
    await vi.advanceTimersByTimeAsync(0);

    expect(scheduled.mock.calls.length).toBe(timers);
    expect(container.querySelector(".org-src")).toBeNull();

    vi.unstubAllGlobals();
  });

  it("drops a refusal that arrives after the block was removed", async () => {
    vi.useFakeTimers();
    const scheduled = vi.spyOn(window, "setTimeout");
    let refuse: (reason: Error) => void = () => {};
    const writeText = vi.fn(
      () =>
        new Promise<void>((_resolve, reject) => {
          refuse = reject;
        }),
    );
    vi.stubGlobal("navigator", { clipboard: { writeText } });

    const own = document.createElement("p");
    own.textContent = "what the host selected";
    document.body.append(own);

    const { unmount } = render(() => <SourceBlock lang="sh" code="ls -la" />);
    screen.getByRole("button", { name: "Copy code to clipboard" }).click();

    const range = document.createRange();
    range.selectNodeContents(own);
    const selection = window.getSelection();
    selection?.removeAllRanges();
    selection?.addRange(range);

    unmount();
    const timers = scheduled.mock.calls.length;
    refuse(new Error("denied"));
    await vi.advanceTimersByTimeAsync(0);

    expect(scheduled.mock.calls.length).toBe(timers);
    expect(window.getSelection()?.toString()).toBe("what the host selected");

    own.remove();
    vi.unstubAllGlobals();
  });

  it("announces the outcome politely", () => {
    const { container } = render(() => <SourceBlock lang="rust" code="fn f() {}" />);
    expect(container.querySelector(".org-src__chrome")).toHaveAttribute(
      "aria-live",
      "polite",
    );
  });
});
