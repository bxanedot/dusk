import { useEffect, useRef, useState } from "react";

const sections = [
  ["account", "Account"], ["profiles", "Profiles & saves"], ["appearance", "Appearance"],
  ["detection", "Game detection"], ["installers", "Installers"], ["updates", "Updates"],
  ["data", "Local data"], ["console", "Console mode"], ["presence", "Discord presence"], ["about", "About Dusk"],
];

export function SettingsNavigation({ guestMode }: { guestMode: boolean }) {
  const [active, setActive] = useState(guestMode ? "profiles" : "account");
  const nav = useRef<HTMLElement>(null);
  useEffect(() => {
    const scroller = nav.current?.closest(".content");
    if (!scroller) return;
    const update = () => {
      const top = scroller.getBoundingClientRect().top + 48;
      const candidates = sections.map(([id]) => document.getElementById("settings-" + id)).filter((el): el is HTMLElement => Boolean(el));
      // DOM position matters because the existing settings order differs from navigation grouping.
      candidates.sort((a, b) => a.getBoundingClientRect().top - b.getBoundingClientRect().top);
      const current = candidates.filter(el => el.getBoundingClientRect().top <= top).at(-1) || candidates[0];
      if (current) setActive(current.id.replace("settings-", ""));
    };
    scroller.addEventListener("scroll", update, { passive: true });
    update();
    return () => scroller.removeEventListener("scroll", update);
  }, [guestMode]);
  return <nav ref={nav} className="settings-nav" aria-label="Settings sections">
    <span className="settings-nav-heading">Preferences</span>
    {sections.filter(([id]) => id !== "account" || !guestMode).map(([id, label]) =>
      <button key={id} type="button" className={"settings-nav-item" + (active === id ? " active" : "")}
        aria-current={active === id ? "location" : undefined} onClick={() => {
          setActive(id);
          const section = document.getElementById("settings-" + id);
          section?.scrollIntoView({ behavior: "instant", block: "start" });
          section?.focus({ preventScroll: true });
        }}>{label}</button>)}
  </nav>;
}
