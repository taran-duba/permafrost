import React, { lazy, Suspense } from "react";
import ReactDOM from "react-dom/client";
import "@fontsource-variable/libre-franklin";
import "@fontsource-variable/space-grotesk";
import "@fontsource-variable/jetbrains-mono";

// One codebase, two apps: `vite --mode lite` builds Permafrost Lite (the CacheCAD-style
// upload/download app); the default build is Permafrost. Each loads only its own UI.
const Root = lazy(() => (import.meta.env.MODE === "lite" ? import("./lite/LiteApp") : import("./App")));

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Suspense fallback={null}>
      <Root />
    </Suspense>
  </React.StrictMode>,
);
