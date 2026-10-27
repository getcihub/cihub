import type { ComponentProps } from "react";

import type { SetupState } from "@/client/types.gen";
import { Badge, Spinner } from "@cihub/ui";

type Tone = NonNullable<ComponentProps<typeof Badge>["tone"]>;

const SETUP_STATES: Record<Exclude<SetupState, "checking">, { label: string; tone: Tone }> = {
  started: { label: "Queue started", tone: "success" },
  ready: { label: "Ready", tone: "success" },
  ready_with_warnings: { label: "Ready, with a warning", tone: "warning" },
  cannot_merge: { label: "Cannot merge", tone: "destructive" },
  invalid_config: { label: "Invalid .cihub.yaml", tone: "destructive" },
};

/** The verdict of the setup of a repository, as the server computes it; a read that runs turns. */
export function SetupStateBadge({ state }: { state: SetupState }) {
  if (state === "checking") {
    return (
      <span className="inline-flex items-center gap-1.5 text-md text-muted-foreground">
        <Spinner className="size-3" />
        Checking
      </span>
    );
  }
  const { label, tone } = SETUP_STATES[state];

  return <Badge tone={tone}>{label}</Badge>;
}
