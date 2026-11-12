import { createRoot } from "react-dom/client";
import "./style.css";
import { boot } from "./api";
import { setStatus } from "./nav";
import App from "./App";
import MobileApp from "./mobile/MobileApp";

// Feel like an app, not a web page: no browser context menu, no reload/print/find keys.
// Our own menus call preventDefault themselves and still open.
addEventListener("contextmenu", (e) => e.preventDefault());
addEventListener("keydown", (e) => {
  const k = e.key.toLowerCase();
  if (k === "f5" || ((e.ctrlKey || e.metaKey) && ["r", "p", "f", "g", "u", "j"].includes(k))) e.preventDefault();
});

// Phone UI on Tauri mobile; `?mobile` shows it in the desktop preview.
const mobile = /android|iphone|ipad/i.test(navigator.userAgent) || new URLSearchParams(location.search).has("mobile");
createRoot(document.getElementById("app")!).render(mobile ? <MobileApp /> : <App />);
// The desktop App holds the splash until its local service answers; the phone has none.
if (mobile) {
  const s = document.getElementById("splash");
  setTimeout(() => { s?.classList.add("gone"); s?.addEventListener("transitionend", () => s.remove(), { once: true }); }, 600);
}
boot().catch((e) => setStatus(`Could not start: ${e}`, "bad"));
