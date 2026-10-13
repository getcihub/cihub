import { useSuspenseQuery } from "@tanstack/react-query";

import type { Queue } from "@/client/types.gen";
import { orgQueuesOptions } from "@/domain/queue/queries";

/** The queues of `org`, from the read that the layout of the organization starts. */
export function useOrgQueues(org: string): Queue[] {
  return useSuspenseQuery(orgQueuesOptions(org)).data.queues;
}
