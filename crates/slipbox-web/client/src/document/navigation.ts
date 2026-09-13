import { referenceOf, type LinkTarget, type Navigation } from "../org/navigation.jsx";
import type {
  OrgDocumentIntent,
  OrgDocumentLink,
  OrgDocumentOptions,
} from "./contract.js";

function linkOf(target: LinkTarget): OrgDocumentLink {
  return { id: target.id, target: target.target, reference: referenceOf(target) };
}

export interface DocumentNavigation {
  readonly navigation: Navigation;
  /** Withdraw a preview still raised, to the hook that raised it. */
  readonly retire: () => void;
}

export function documentNavigation(
  options: () => OrgDocumentOptions,
): DocumentNavigation {
  const emit = (intent: OrgDocumentIntent): void => {
    options().onIntent?.(intent);
  };
  // Dismissal belongs to the hook that received the preview.
  let raisedWith: ((intent: OrgDocumentIntent) => void) | undefined;
  const withdraw = (): void => {
    const owed = raisedWith;
    raisedWith = undefined;
    owed?.({ verb: "dismiss" });
  };
  const navigation: Navigation = {
    href: (target) => options().href?.(linkOf(target)) ?? null,
    glance: (request) => {
      if (request === null) {
        const owed = raisedWith ?? options().onIntent;
        raisedWith = undefined;
        owed?.({ verb: "dismiss" });
        return;
      }
      raisedWith = options().onIntent;
      emit({
        verb: "glance",
        link: linkOf(request.target),
        origin: request.origin,
        gesture: request.gesture,
      });
    },
    pin: (target) => emit({ verb: "pin", link: linkOf(target) }),
    go: (target) => emit({ verb: "go", link: linkOf(target) }),
  };
  return { navigation, retire: withdraw };
}
