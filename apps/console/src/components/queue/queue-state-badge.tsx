import type { ComponentProps } from "react";

import type { Queue } from "@/client/types.gen";
import { Badge } from "@cihub/ui/components/badge";

type Tone = NonNullable<ComponentProps<typeof Badge>["tone"]>;

const QUEUE_STATES: Record<string, { label: string; tone: Tone }> = {
  running: { label: "Running", tone: "success" },
  paused: { label: "Paused", tone: "warning" },
  draining: { label: "Draining", tone: "info" },
  switching_modes: { label: "Switching modes", tone: "info" },
};

/** The state of a queue, unless something stops it: a lost access, a file that is not valid. */
export function QueueStateBadge({ queue }: { queue: Queue }) {
  if (queue.is_suspended) {
    return <Badge tone="destructive">Access lost</Badge>;
  }
  if (queue.config_error) {
    return <Badge tone="destructive">Invalid .cihub.yaml</Badge>;
  }
  const { label, tone } = QUEUE_STATES[queue.state] ?? { label: queue.state, tone: "neutral" };

  return <Badge tone={tone}>{label}</Badge>;
}
