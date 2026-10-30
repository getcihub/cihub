import { Brand } from "@/components/session/parts";
import { useMe } from "@/hooks/use-me";
import { DeviceForm } from "@/pages/device/components/device-form";

/** The page where a person approves the code that their command line shows. */
export function DevicePage() {
  const me = useMe();

  return (
    <div className="flex min-h-screen items-center justify-center bg-card px-8 py-12">
      <div className="w-full max-w-90">
        <Brand className="mb-9" />
        <DeviceForm login={me.login} />
      </div>
    </div>
  );
}
