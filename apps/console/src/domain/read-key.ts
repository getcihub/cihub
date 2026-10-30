import type { QueryKey } from "@tanstack/react-query";

import type * as sdk from "@/client/sdk.gen";

/** An operation of the API, as the generated client names it in the `_id` of a query key. */
export type Operation = keyof typeof sdk;

/** What the dashboard reads in a query key of the generated client: the operation and the path of the read. */
export interface ReadKey {
  _id?: Operation;
  path?: { org?: string; repo?: string; queue?: string };
}

/**
 * The fields of `queryKey`, a key of the generated client: its first entry holds them. Only for a predicate that a
 * key built by a `*QueryKey` function cannot say, as "every read of an organization".
 */
export function readKeyOf(queryKey: QueryKey): ReadKey {
  return queryKey[0] ?? {};
}
