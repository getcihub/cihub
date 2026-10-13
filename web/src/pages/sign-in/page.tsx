import { getRouteApi } from "@tanstack/react-router";

import { NoOrganization } from "@/pages/sign-in/components/no-organization";
import { Brand } from "@/pages/sign-in/components/parts";
import { SignInForm } from "@/pages/sign-in/components/sign-in-form";
import { SignInHero } from "@/pages/sign-in/components/sign-in-hero";

const route = getRouteApi("/sign-in");

/** The sign-in, or the way out for a person whose account is in no organization of the App. */
export function SignInPage() {
  const { me } = route.useLoaderData();
  const { error, next } = route.useSearch();

  return (
    <div className="grid min-h-screen grid-cols-[1.1fr_1fr] max-lg:grid-cols-1">
      <SignInHero />
      <section className="flex flex-col items-center justify-center bg-card px-8 py-12">
        <div className="w-full max-w-90">
          {/* One logo: the hero shows it; the form shows it only when the hero is hidden. */}
          <Brand className="hidden max-lg:mb-9 max-lg:flex" />
          {me ? (
            <NoOrganization installUrl={me.app_install_url ?? null} />
          ) : (
            <SignInForm hasFailed={error === "failed"} next={next ?? "/"} />
          )}
          <div className="my-7 h-px bg-divider" />
          <p className="text-md text-muted-foreground">
            Scripts and the <code>cihub</code> CLI sign in with an API token instead.
          </p>
        </div>
      </section>
    </div>
  );
}
