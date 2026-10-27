import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import "@/styles/globals.css";
import type { ViewerData } from "./data";
import { Viewer } from "./viewer";

const data = JSON.parse(document.getElementById("viewer-data")?.textContent ?? "") as ViewerData;
const root = document.getElementById("root");

if (root) {
  createRoot(root).render(
    <StrictMode>
      <Viewer data={data} />
    </StrictMode>,
  );
}
