// Keep this file tiny and import nothing statically: the inline splash in index.html paints first,
// then React, the stylesheet and the one UI we need load in parallel behind it.

// Feel like an app, not a web page: no browser context menu, no reload/print/find keys.
// Our own menus call preventDefault themselves and still open.
addEventListener("contextmenu", (e) => e.preventDefault());
addEventListener("keydown", (e) => {
  const k = e.key.toLowerCase();
  if (k === "f5" || ((e.ctrlKey || e.metaKey) && ["r", "p", "f", "g", "u", "j"].includes(k))) e.preventDefault();
});

// Phone UI on Tauri mobile; `?mobile` shows it in the desktop preview.
const mobile = /android|iphone|ipad/i.test(navigator.userAgent) || new URLSearchParams(location.search).has("mobile");

const [{ createRoot }, { default: Root }, { boot }, { setStatus }] = await Promise.all([
  import("react-dom/client"),
  mobile ? import("./mobile/MobileApp") : import("./App"),
  import("./api"),
  import("./nav"),
  import("./style.css"),
]);

createRoot(document.getElementById("app")!).render(<Root />);
// The desktop App holds the splash until its local service answers; the phone has none.
if (mobile) {
  const s = document.getElementById("splash");
  setTimeout(() => { s?.classList.add("gone"); s?.addEventListener("transitionend", () => s.remove(), { once: true }); }, 600);
}
boot().catch((e) => setStatus(`Could not start: ${e}`, "bad"));
