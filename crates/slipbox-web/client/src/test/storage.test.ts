import { expect, it } from "vitest";

it.each(["localStorage", "sessionStorage"] as const)(
  "supplies browser %s in DOM tests",
  (name) => {
    const storage = window[name];
    expect(storage).toBeInstanceOf(Storage);
    storage.setItem("slipbox-storage-test", "present");
    expect(storage.getItem("slipbox-storage-test")).toBe("present");
    storage.removeItem("slipbox-storage-test");
    expect(storage.getItem("slipbox-storage-test")).toBeNull();
  },
);
