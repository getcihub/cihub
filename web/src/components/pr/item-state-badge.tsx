import type { ComponentProps } from "react";

import { Badge } from "@/components/ui/badge";
import { itemStateLabel } from "@/domain/pr/words";

type Tone = NonNullable<ComponentProps<typeof Badge>["tone"]>;

interface ItemStateLook {
  tone: Tone;
  /** A ring: a state that waits. */
  isRing: boolean;
}

// The looks of the mockups v5.
const ITEM_STATES: Record<string, ItemStateLook> = {
  not_ready: { tone: "neutral", isRing: true },
  pending: { tone: "neutral", isRing: true },
  testing: { tone: "info", isRing: true },
  pending_failure: { tone: "warning", isRing: false },
  tests_passed: { tone: "success", isRing: false },
  merged: { tone: "info", isRing: false },
  failed: { tone: "destructive", isRing: false },
  cancelled: { tone: "neutral", isRing: false },
};

export function ItemStateBadge({ state }: { state: string }) {
  const { tone, isRing } = ITEM_STATES[state] ?? { tone: "neutral", isRing: false };

  return (
    <Badge tone={tone} isRing={isRing}>
      {itemStateLabel(state)}
    </Badge>
  );
}
