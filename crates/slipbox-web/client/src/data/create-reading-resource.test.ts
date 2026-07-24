import { createRoot, createSignal } from "solid-js";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { createReadingResource } from "./create-reading-resource.js";
import { __resetRefocusForTests } from "./refetch-on-focus.js";

/** Yield to the microtask queue so a settled fetch reaches the resource. */
function settle(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

/** A fetcher whose promises are resolved by hand, indexed by call order. */
function deferred<T>() {
  const resolvers: ((value: T) => void)[] = [];
  const rejecters: ((reason: unknown) => void)[] = [];
  const fetcher = (): Promise<T> =>
    new Promise<T>((resolve, reject) => {
      resolvers.push(resolve);
      rejecters.push(reject);
    });
  return {
    fetcher,
    resolve: (index: number, value: T) => resolvers[index]!(value),
    reject: (index: number, reason: unknown) => rejecters[index]!(reason),
    count: () => resolvers.length,
  };
}

describe("createReadingResource", () => {
  beforeEach(() => {
    __resetRefocusForTests();
  });

  afterEach(() => {
    __resetRefocusForTests();
  });

  it("resolves a keyed source to its value", async () => {
    await createRoot(async (dispose) => {
      const [key] = createSignal("a");
      const resource = createReadingResource(key, async (k) => `value-${k}`);
      expect(resource.ready()).toBeUndefined();
      await settle();
      expect(resource.ready()).toBe("value-a");
      expect(resource.error()).toBeUndefined();
      dispose();
    });
  });

  it("does not fetch while the source is falsy", async () => {
    await createRoot(async (dispose) => {
      const [key] = createSignal<string | false>(false);
      let calls = 0;
      const resource = createReadingResource(key, async (k) => {
        calls += 1;
        return k;
      });
      await settle();
      expect(calls).toBe(0);
      expect(resource.ready()).toBeUndefined();
      dispose();
    });
  });

  it("withholds the previous key's value while the next key is in flight", async () => {
    await createRoot(async (dispose) => {
      const [key, setKey] = createSignal("a");
      const resource = createReadingResource(key, async (k) => `value-${k}`);
      await settle();
      expect(resource.ready()).toBe("value-a");

      setKey("b");
      expect(resource.ready()).toBeUndefined();
      expect(resource.loading()).toBe(true);

      await settle();
      expect(resource.ready()).toBe("value-b");
      dispose();
    });
  });

  it("keeps the resolved value across a refetch of the same key", async () => {
    await createRoot(async (dispose) => {
      const [key] = createSignal("a");
      const pending = deferred<string>();
      const resource = createReadingResource(key, pending.fetcher);
      pending.resolve(0, "first");
      await settle();
      expect(resource.ready()).toBe("first");

      resource.refetch();
      expect(resource.ready()).toBe("first");
      pending.resolve(1, "second");
      await settle();
      expect(resource.ready()).toBe("second");
      dispose();
    });
  });

  it("pairs the value with its own key when requests settle out of order", async () => {
    await createRoot(async (dispose) => {
      const [key, setKey] = createSignal("a");
      const pending = deferred<string>();
      const resource = createReadingResource(key, pending.fetcher);
      setKey("b");
      await settle();
      expect(pending.count()).toBe(2);

      // Index 1 is the request for "b", index 0 the stale one for "a".
      pending.resolve(1, "value-b");
      await settle();
      expect(resource.ready()).toBe("value-b");
      pending.resolve(0, "value-a");
      await settle();
      expect(resource.ready()).toBe("value-b");
      dispose();
    });
  });

  it("reports a failure without throwing, and clears it on the next key", async () => {
    await createRoot(async (dispose) => {
      const [key, setKey] = createSignal("bad");
      const resource = createReadingResource(key, async (k) => {
        if (k === "bad") {
          throw new Error("not in the slipbox");
        }
        return `value-${k}`;
      });
      await settle();
      expect(resource.ready()).toBeUndefined();
      expect((resource.error() as Error).message).toBe("not in the slipbox");

      setKey("good");
      expect(resource.error()).toBeUndefined();
      await settle();
      expect(resource.ready()).toBe("value-good");
      expect(resource.error()).toBeUndefined();
      dispose();
    });
  });
});
