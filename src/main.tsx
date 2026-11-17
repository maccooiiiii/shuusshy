import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";
import { IS_MAC } from "./lib/platform";

// macOS: rounded glass corners (index.css).
if (IS_MAC) document.documentElement.classList.add("mac");

async function start() {
  // Development only: `?demo=win|mac` runs the page with sample data (src/dev/demo.ts), used to
  // take the user guide's screenshots. Production builds drop this branch entirely.
  let stage: (() => void) | null = null;
  if (import.meta.env.DEV && new URLSearchParams(location.search).has("demo")) {
    const demo = await import("./dev/demo");
    demo.installDemo();
    stage = demo.stageDemo;
  }
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
  stage?.();
}

void start();
