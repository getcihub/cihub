import { useEffect } from "react";

/** Sets the title of the tab of the browser to `title` while the component shows. */
export function useDocumentTitle(title: string): void {
  useEffect(() => {
    document.title = title;
  }, [title]);
}
