import { createMemo, createSignal } from "solid-js";
import { render } from "solid-js/web";

import { AssetProvider } from "../org/assets.jsx";
import { NavigationProvider } from "../org/navigation.jsx";
import { parseOrg } from "../org/parse.js";
import { RenderDocument } from "../org/RenderDocument.jsx";
import type { OrgDocumentHandle, OrgDocumentOptions } from "./contract.js";
import { documentNavigation } from "./navigation.js";

import "../styles/tokens.css";
import "../org/org.css";
import "./document.css";

const HOST_CLASS = "org-document-host";

export function mountOrgDocument(
  host: Element,
  options: OrgDocumentOptions,
): OrgDocumentHandle {
  const [mounted, setMounted] = createSignal<OrgDocumentOptions>(options);

  const container = host.ownerDocument.createElement("div");
  container.className = HOST_CLASS;
  host.append(container);

  const applyTheme = (): void => {
    const theme = mounted().theme ?? "system";
    if (theme === "system") {
      delete container.dataset.theme;
    } else {
      container.dataset.theme = theme;
    }
  };
  applyTheme();

  const { navigation, retire } = documentNavigation(mounted);
  const resolveAsset = (target: string): string | null =>
    mounted().resolveAsset?.(target) ?? null;

  const dispose = render(() => {
    // Option-only updates must preserve the elements held by a preview.
    const source = createMemo(() => mounted().content.source);
    const baseLevel = createMemo(() => mounted().content.baseLevel);
    const parsed = createMemo(() => parseOrg(source()));
    return (
      <NavigationProvider navigation={navigation}>
        <AssetProvider resolve={resolveAsset}>
          <RenderDocument document={parsed()} baseLevel={baseLevel()} />
        </AssetProvider>
      </NavigationProvider>
    );
  }, container);

  const rebuilds = (patch: Partial<OrgDocumentOptions>): boolean => {
    const next = patch.content;
    const held = mounted().content;
    return (
      next !== undefined &&
      (next.source !== held.source || next.baseLevel !== held.baseLevel)
    );
  };

  let live = true;
  return {
    update: (patch) => {
      if (!live) {
        return;
      }
      // Retire through the old hook before replacing it or its preview origin.
      const replacesHook =
        "onIntent" in patch && patch.onIntent !== mounted().onIntent;
      if (rebuilds(patch) || replacesHook) {
        retire();
      }
      setMounted((previous) => ({
        ...previous,
        ...patch,
        content: patch.content ?? previous.content,
      }));
      applyTheme();
    },
    dispose: () => {
      if (!live) {
        return;
      }
      live = false;
      retire();
      dispose();
      container.remove();
    },
  };
}
