import { Page } from "@/components/ui/page";
import { LoadingStatus } from "@/components/ui/skeleton";
import { TopBar, TopBarLogo } from "./frame";

/** A page outside an organization reads its data (the sign-in, `/`): the top bar, empty, so that it does not jump. */
export function PendingPage() {
  return (
    <>
      <TopBar>
        <TopBarLogo />
      </TopBar>
      <Page aria-busy="true">
        <LoadingStatus />
      </Page>
    </>
  );
}
