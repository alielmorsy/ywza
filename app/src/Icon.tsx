// SVG icon sprite from mockup-ui/index.html. Render <Sprite/> once, then <Icon name="x"/>.

export type IconName = 'laptop' | 'monitor' | 'phone' | 'server' | 'sliders' | 'back' | 'copy' | 'refresh' | 'qr' | 'chat' | 'send' | 'play' | 'pause' | 'stop' | 'expand' | 'shrink' | 'pointer' | 'check' | 'chev' | 'shield' | 'warn' | 'tool' | 'trash' | 'eye' | 'min' | 'max' | 'home' | 'plus' | 'pencil' | 'dock' | 'split' | 'window' | 'bug' | 'star' | 'link' | 'unlink' | 'spark' | 'keyboard' | 'x';

const SYMBOLS = `<symbol id="i-laptop" viewBox="0 0 24 24"><rect x="4" y="5" width="16" height="11" rx="1.5"/><path d="M2 19h20"/></symbol>
<symbol id="i-monitor" viewBox="0 0 24 24"><rect x="3" y="4" width="18" height="12" rx="1.5"/><path d="M8 20h8M12 16v4"/></symbol>
<symbol id="i-phone" viewBox="0 0 24 24"><rect x="7" y="3" width="10" height="18" rx="2"/><path d="M11 18h2"/></symbol>
<symbol id="i-server" viewBox="0 0 24 24"><rect x="4" y="4" width="16" height="6" rx="1.5"/><rect x="4" y="14" width="16" height="6" rx="1.5"/><path d="M8 7h.01M8 17h.01"/></symbol>
<symbol id="i-sliders" viewBox="0 0 24 24"><path d="M4 7h10M18 7h2M4 17h2M10 17h10"/><circle cx="16" cy="7" r="2"/><circle cx="8" cy="17" r="2"/></symbol>
<symbol id="i-back" viewBox="0 0 24 24"><path d="M15 5l-7 7 7 7"/></symbol>
<symbol id="i-copy" viewBox="0 0 24 24"><rect x="8" y="8" width="12" height="12" rx="2"/><path d="M16 8V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h2"/></symbol>
<symbol id="i-refresh" viewBox="0 0 24 24"><path d="M20 11a8 8 0 1 0-2.3 5.7"/><path d="M20 5v6h-6"/></symbol>
<symbol id="i-qr" viewBox="0 0 24 24"><rect x="4" y="4" width="6" height="6"/><rect x="14" y="4" width="6" height="6"/><rect x="4" y="14" width="6" height="6"/><path d="M14 14h2v2h-2zM18 18h2M14 20h2M20 14v2"/></symbol>
<symbol id="i-chat" viewBox="0 0 24 24"><path d="M4 5h16v11H9l-5 4z"/></symbol>
<symbol id="i-send" viewBox="0 0 24 24"><path d="M4 12l16-8-6 16-3-7z"/></symbol>
<symbol id="i-play" viewBox="0 0 24 24"><path d="M7 5l12 7-12 7z"/></symbol>
<symbol id="i-pause" viewBox="0 0 24 24"><path d="M8 5v14M16 5v14"/></symbol>
<symbol id="i-stop" viewBox="0 0 24 24"><rect x="6" y="6" width="12" height="12" rx="1.5"/></symbol>
<symbol id="i-expand" viewBox="0 0 24 24"><path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5"/></symbol>
<symbol id="i-shrink" viewBox="0 0 24 24"><path d="M9 4v5H4M15 4v5h5M9 20v-5H4M15 20v-5h5"/></symbol>
<symbol id="i-pointer" viewBox="0 0 24 24"><path d="M6 4l12 7-5 1.5L11 18z"/></symbol>
<symbol id="i-check" viewBox="0 0 24 24"><path d="M5 12l5 5 9-10"/></symbol>
<symbol id="i-chev" viewBox="0 0 24 24"><path d="M6 9l6 6 6-6"/></symbol>
<symbol id="i-shield" viewBox="0 0 24 24"><path d="M12 3l8 3v6c0 5-3.5 8-8 9-4.5-1-8-4-8-9V6z"/></symbol>
<symbol id="i-warn" viewBox="0 0 24 24"><path d="M12 4l9 16H3z"/><path d="M12 10v4M12 17h.01"/></symbol>
<symbol id="i-tool" viewBox="0 0 24 24"><path d="M14 6a4 4 0 0 0 5 5l-9 9a2 2 0 0 1-3-3l9-9z"/></symbol>
<symbol id="i-trash" viewBox="0 0 24 24"><path d="M5 7h14M10 7V4h4v3M7 7l1 13h8l1-13"/></symbol>
<symbol id="i-eye" viewBox="0 0 24 24"><path d="M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/></symbol>
<symbol id="i-min" viewBox="0 0 12 12"><path d="M1 6h10"/></symbol>
<symbol id="i-max" viewBox="0 0 12 12"><rect x="1.5" y="1.5" width="9" height="9" rx="1"/></symbol>
<symbol id="i-home" viewBox="0 0 24 24"><path d="M4 11l8-7 8 7v9h-5v-6H9v6H4z"/></symbol>
<symbol id="i-plus" viewBox="0 0 24 24"><path d="M12 5v14M5 12h14"/></symbol>
<symbol id="i-pencil" viewBox="0 0 24 24"><path d="M4 20l1-4L16 5l3 3L8 19z"/></symbol>
<symbol id="i-dock" viewBox="0 0 24 24"><rect x="3" y="4" width="18" height="16" rx="2"/><path d="M15 4v16"/></symbol>
<symbol id="i-split" viewBox="0 0 24 24"><rect x="3" y="4" width="18" height="16" rx="2"/><path d="M12 4v16"/></symbol>
<symbol id="i-window" viewBox="0 0 24 24"><rect x="3" y="4" width="18" height="16" rx="2"/><path d="M3 9h18M6.5 6.5h.01M9 6.5h.01"/></symbol>
<symbol id="i-bug" viewBox="0 0 24 24"><rect x="7" y="8" width="10" height="12" rx="5"/><path d="M12 12v8M9 5l1.5 3M15 5l-1.5 3M3 13h4M17 13h4M4 19l3-2M20 19l-3-2M4 7l3 2M20 7l-3 2"/></symbol>
<symbol id="i-star" viewBox="0 0 24 24"><path d="M12 4l2.4 5 5.4.6-4 3.7 1.1 5.4L12 16l-4.9 2.7 1.1-5.4-4-3.7 5.4-.6z"/></symbol>
<symbol id="i-link" viewBox="0 0 24 24"><path d="M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1"/><path d="M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1"/></symbol>
<symbol id="i-unlink" viewBox="0 0 24 24"><path d="M14.5 6.5l1-1a4 4 0 0 1 5.7 5.7l-1 1M9.5 17.5l-1 1a4 4 0 0 1-5.7-5.7l1-1M4 4l16 16"/></symbol>
<symbol id="i-spark" viewBox="0 0 24 24"><path d="M12 3v4M12 17v4M3 12h4M17 12h4M6 6l2.5 2.5M15.5 15.5L18 18M6 18l2.5-2.5M15.5 8.5L18 6"/></symbol>
<symbol id="i-keyboard" viewBox="0 0 24 24"><rect x="2.5" y="6" width="19" height="12" rx="2"/><path d="M6 10h.01M10 10h.01M14 10h.01M18 10h.01M7 14h10"/></symbol>
<symbol id="i-x" viewBox="0 0 12 12"><path d="M1.5 1.5l9 9M10.5 1.5l-9 9"/></symbol>`;

export const Sprite = () => (
  <svg width="0" height="0" style={{ position: "absolute" }} aria-hidden dangerouslySetInnerHTML={{ __html: `<defs>${SYMBOLS}</defs>` }} />
);

export const Icon = ({ name, className = "" }: { name: IconName; className?: string }) => (
  <svg className={`i ${className}`}><use href={`#i-${name}`} /></svg>
);
