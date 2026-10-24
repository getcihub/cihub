import { SiteFooter } from "./site-footer";
import { SiteHeader } from "./site-header";

import "./home.css";

export default function Layout({ children }: LayoutProps<"/">) {
  return (
    <div className="home">
      <SiteHeader />
      {children}
      <SiteFooter />
    </div>
  );
}
