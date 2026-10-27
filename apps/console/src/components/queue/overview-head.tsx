import type { ReactNode } from "react";

import { PageHeader, PageLocation, PageTitle } from "@cihub/ui";

/** The head of a list: where it is, then its title, and its actions on the right. */
export function OverviewHead({
  where,
  title,
  children,
}: {
  where: string;
  title: string;
  children?: ReactNode;
}) {
  return (
    <PageHeader>
      <div>
        <PageLocation>{where}</PageLocation>
        <PageTitle>{title}</PageTitle>
      </div>
      {children && <div className="ml-auto flex items-center gap-2">{children}</div>}
    </PageHeader>
  );
}
