import type { QueryClient } from "@tanstack/react-query";

import { userOptions, userOrgsOptions } from "./queries";

/** Forgets the person of the session: a read still fresh in the cache would send them back from the sign-in. */
export function forgetMe(queryClient: QueryClient): void {
  queryClient.removeQueries({ queryKey: userOptions().queryKey });
  queryClient.removeQueries({ queryKey: userOrgsOptions().queryKey });
}

/** Reads again the organizations of the person: CIHub read their installations again. */
export async function invalidateUserOrgs(queryClient: QueryClient): Promise<void> {
  await queryClient.invalidateQueries({ queryKey: userOrgsOptions().queryKey });
}
