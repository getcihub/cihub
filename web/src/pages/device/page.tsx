import { getRouteApi } from "@tanstack/react-router";

import { Brand } from "@/components/session/parts";
import { DeviceForm } from "@/pages/device/components/device-form";

const route = getRouteApi("/device");

/** The page where a person approves the code that their command line shows. */
export function DevicePage() {
  const { me } = route.useLoaderData();

  return (
    <div className="flex min-h-screen items-center justify-center bg-card px-8 py-12">
      <div className="w-full max-w-90">
        <Brand className="mb-9" />
        <DeviceForm login={me.login} />
      </div>
    </div>
  );
}
