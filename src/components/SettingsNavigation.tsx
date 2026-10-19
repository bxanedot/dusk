import { useEffect, useRef, useState } from "react";

const sections = [
  ["profiles", "Profiles & saves"], ["steam", "Steam achievements"],
  ["presence", "Discord presence"], ["account", "Account"], ["console", "Console mode"],
  ["appearance", "Appearance"], ["updates", "Updates"], ["data", "Local data"],
  ["installers", "Installers"], ["detection", "Game detection"], ["about", "About Dusk"],
];

export function SettingsNavigation({ guestMode }: { guestMode: boolean }) {
  const [active, setActive] = useState("profiles");
  const nav = useRef<HTMLElement>(null);
  useEffect(() => {
    const scroller = nav.current?.closest(".content");
    if (!scroller) return;
    const update = () => {
      const top = scroller.getBoundingClientRect().top + 48;
      const candidates = sections.map(([id]) => document.getElementById("settings-" + id)).filter((el): el is HTMLElement => Boolean(el));
      // Keep the active item aligned with the card currently at the top of the scroll area.
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
          const collapsible = section instanceof HTMLDetailsElement
            ? section
            : section?.querySelector("details");
          if (collapsible) collapsible.open = true;
          section?.scrollIntoView({ behavior: "instant", block: "start" });
          section?.focus({ preventScroll: true });
        }}>{label}</button>)}
  </nav>;
}
