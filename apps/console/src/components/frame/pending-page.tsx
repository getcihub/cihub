import { Page } from "@cihub/ui/components/page";
import { LoadingStatus } from "@cihub/ui/components/skeleton";
import { TopBar, TopBarLogo } from "./top-bar";

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
