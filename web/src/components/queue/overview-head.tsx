import { PageHeader, PageLocation, PageTitle } from "@/components/ui/page";

/** The head of a list of queues: where they are, then the title. */
export function OverviewHead({ where }: { where: string }) {
  return (
    <PageHeader>
      <div>
        <PageLocation>{where}</PageLocation>
        <PageTitle>Queues</PageTitle>
      </div>
    </PageHeader>
  );
}
