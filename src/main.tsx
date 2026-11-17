import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";
import { IS_MAC } from "./lib/platform";

// macOS: rounded glass corners (index.css).
if (IS_MAC) document.documentElement.classList.add("mac");

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
