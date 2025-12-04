import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./globals.css";

if (typeof window !== "undefined") {
  window.addEventListener("contextmenu", (event) => event.preventDefault());
  window.addEventListener(
    "wheel",
    (event) => {
      if (event.ctrlKey) {
        event.preventDefault();
      }
    },
    { passive: false }
  );
  window.addEventListener("keydown", (event) => {
    if (
      (event.ctrlKey || event.metaKey) &&
      ["=", "+", "-", "_", "0"].includes(event.key)
    ) {
      event.preventDefault();
    }
  });
  ["gesturestart", "gesturechange", "gestureend"].forEach((type) => {
    window.addEventListener(type, (event) => event.preventDefault());
  });
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
