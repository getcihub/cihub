import { Card } from "@cihub/ui/components/card";
import { Spinner } from "@cihub/ui/components/spinner";

/** The wait for a read of the repositories that the person reaches: the first one, or one that they asked for. */
export function Reading({ org, isFirstRead }: { org: string; isFirstRead: boolean }) {
  return (
    <Card className="flex items-start gap-3 p-4" role="status">
      <Spinner className="mt-1 size-4" />
      <div>
        <h2 className="text-xl font-semibold">Reading your repositories</h2>
        <p className="text-md text-secondary-foreground">
          {isFirstRead
            ? `One moment: CIHub reads on GitHub the repositories of ${org} that your account reaches.`
            : `CIHub reads again on GitHub the repositories of ${org} that your account reaches.`}
        </p>
      </div>
    </Card>
  );
}

/** The wait after an install: the server signed the person in again and connected the organization. */
export function Connecting({ org, isUpdate }: { org: string; isUpdate: boolean }) {
  return (
    <Card className="flex items-start gap-3 p-4" role="status">
      <Spinner className="mt-1 size-4" />
      <div>
        <h2 className="text-xl font-semibold">{isUpdate ? `Updating ${org}` : `Connecting ${org}`}</h2>
        <p className="text-md text-secondary-foreground">
          {isUpdate
            ? "The repositories of the App changed on GitHub: CIHub reads them and their setup."
            : "One moment: CIHub reads the repositories of the App and their setup."}
        </p>
        <p className="mt-1 text-md text-muted-foreground">
          The link that brought you here proves nothing by itself: CIHub asked GitHub which installations your
          account can reach.
        </p>
      </div>
    </Card>
  );
}
