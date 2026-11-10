import { createRoot } from "react-dom/client";
import "./style.css";
import { boot } from "./api";
import { setStatus } from "./nav";
import App from "./App";

// Feel like an app, not a web page: no browser context menu, no reload/print/find keys.
// Our own menus call preventDefault themselves and still open.
addEventListener("contextmenu", (e) => e.preventDefault());
addEventListener("keydown", (e) => {
  const k = e.key.toLowerCase();
  if (k === "f5" || ((e.ctrlKey || e.metaKey) && ["r", "p", "f", "g", "u", "j"].includes(k))) e.preventDefault();
});

createRoot(document.getElementById("app")!).render(<App />);
boot().catch((e) => setStatus(`Could not start: ${e}`, "bad"));
