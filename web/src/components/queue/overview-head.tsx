import { PageHeader, PageLocation, PageTitle } from "@/components/ui/page";

/** The head of a list: where it is, then its title. */
export function OverviewHead({ where, title }: { where: string; title: string }) {
  return (
    <PageHeader>
      <div>
        <PageLocation>{where}</PageLocation>
        <PageTitle>{title}</PageTitle>
      </div>
    </PageHeader>
  );
}
