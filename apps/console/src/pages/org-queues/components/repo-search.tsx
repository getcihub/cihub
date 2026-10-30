import { getRouteApi } from "@tanstack/react-router";

import { SearchIcon } from "@cihub/icons/search";
import { Input } from "@cihub/ui/components/input";

const route = getRouteApi("/orgs/$org/");

/** The search of the repositories of the organization, which the address keeps. */
export function RepoSearch({ count }: { count: number }) {
  const { q } = route.useSearch();
  const navigate = route.useNavigate();

  return (
    <span className="relative flex items-center">
      <SearchIcon className="pointer-events-none absolute left-2.25 size-3.5 text-muted-foreground" />
      <Input
        className="h-7 w-65 pr-2.5 pl-7 text-base"
        type="search"
        aria-label="Search the repositories"
        placeholder={`Search ${String(count)} repositories`}
        defaultValue={q ?? ""}
        onChange={(event) => {
          const value = event.target.value.trim();
          void navigate({ search: (kept) => ({ ...kept, q: value || undefined }), replace: true });
        }}
      />
    </span>
  );
}
