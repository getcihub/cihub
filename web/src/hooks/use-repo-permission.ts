import { useQuery } from "@tanstack/react-query";

import { getRepoOptions } from "@/client/@tanstack/react-query.gen";
import type { Permission } from "@/client/types.gen";

const NONE: Permission = { read: false, write: false, admin: false };

/**
 * What the person may do on `repo` of `org`, as the server gives it: the buttons follow it. Nothing while it loads
 * or when its read fails: a button that the server would refuse never shows as active.
 */
export function useRepoPermission(org: string, repo: string): Permission {
  const { data } = useQuery(getRepoOptions({ path: { org, repo } }));
  return data?.permission ?? NONE;
}
