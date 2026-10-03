import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";

// Lets the shell render a transparent backdrop inside the native Tauri
// window while keeping a neutral preview backdrop in the browser.
if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
  document.documentElement.classList.add("is-tauri");
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
