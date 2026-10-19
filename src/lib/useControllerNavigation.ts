import { useEffect, useRef, useState } from "react";

type Direction = "up" | "down" | "left" | "right";

interface ControllerNavigationOptions {
  enabled?: boolean;
  onBack?: () => void;
  onToggleConsole?: () => void;
  onPlayGame?: (gameId: string) => void;
}

const BUTTON_A = 0;
const BUTTON_B = 1;
const BUTTON_X = 2;
const BUTTON_START = 9;
const DPAD_UP = 12;
const DPAD_DOWN = 13;
const DPAD_LEFT = 14;
const DPAD_RIGHT = 15;

function focusableElements() {
  return Array.from(
    document.querySelectorAll<HTMLElement>(
      [
        "button:not(:disabled)",
        '[role="button"][tabindex="0"]',
        '[data-controller-focus="true"]',
      ].join(","),
    ),
  ).filter((element) => {
    const style = window.getComputedStyle(element);
    const rect = element.getBoundingClientRect();
    return (
      style.display !== "none" &&
      style.visibility !== "hidden" &&
      rect.width > 0 &&
      rect.height > 0
    );
  });
}

function center(element: HTMLElement) {
  const rect = element.getBoundingClientRect();
  return {
    x: rect.left + rect.width / 2,
    y: rect.top + rect.height / 2,
  };
}

function moveFocus(direction: Direction) {
  const elements = focusableElements();
  if (elements.length === 0) return;

  const active =
    document.activeElement instanceof HTMLElement &&
    elements.includes(document.activeElement)
      ? document.activeElement
      : null;

  if (!active) {
    elements[0].focus({ preventScroll: false });
    elements[0].scrollIntoView({ block: "nearest", inline: "nearest" });
    return;
  }

  const from = center(active);
  let best: { element: HTMLElement; score: number } | null = null;

  for (const element of elements) {
    if (element === active) continue;
    const to = center(element);
    const dx = to.x - from.x;
    const dy = to.y - from.y;

    const valid =
      (direction === "left" && dx < -4) ||
      (direction === "right" && dx > 4) ||
      (direction === "up" && dy < -4) ||
      (direction === "down" && dy > 4);

    if (!valid) continue;

    const primary =
      direction === "left" || direction === "right" ? Math.abs(dx) : Math.abs(dy);
    const secondary =
      direction === "left" || direction === "right" ? Math.abs(dy) : Math.abs(dx);

    // Prefer elements that are mainly in the requested direction rather than
    // diagonally far away, while still allowing movement between UI regions.
    const score = primary + secondary * 2.35;

    if (!best || score < best.score) {
      best = { element, score };
    }
  }

  if (best) {
    best.element.focus({ preventScroll: false });
    best.element.scrollIntoView({
      behavior: "smooth",
      block: "nearest",
      inline: "nearest",
    });
  }
}

function focusedGameId() {
  const active = document.activeElement;
  if (!(active instanceof HTMLElement)) return null;
  const game = active.closest<HTMLElement>("[data-controller-game-id]");
  return game?.dataset.controllerGameId ?? null;
}

export function useControllerNavigation(options: ControllerNavigationOptions) {
  const [connected, setConnected] = useState(false);
  const connectedRef = useRef(false);
  const previousButtons = useRef<boolean[]>([]);
  const repeatState = useRef<{ direction: Direction | null; nextAt: number }>({
    direction: null,
    nextAt: 0,
  });
  const callbacks = useRef(options);
  callbacks.current = options;

  useEffect(() => {
    if (options.enabled === false) return;

    let frame = 0;

    const updateConnected = (value: boolean) => {
      if (connectedRef.current === value) return;
      connectedRef.current = value;
      setConnected(value);
    };

    const onConnected = () => updateConnected(true);
    const onDisconnected = () => {
      updateConnected(navigator.getGamepads().some(Boolean));
      previousButtons.current = [];
    };

    window.addEventListener("gamepadconnected", onConnected);
    window.addEventListener("gamepaddisconnected", onDisconnected);
    updateConnected(navigator.getGamepads().some(Boolean));

    function loop(timestamp: number) {
      const gamepad = navigator.getGamepads().find(Boolean);

      if (gamepad) {
        updateConnected(true);

        const pressed = gamepad.buttons.map((button) => button.pressed);
        const previous = previousButtons.current;
        const justPressed = (index: number) => pressed[index] && !previous[index];

        if (justPressed(BUTTON_A)) {
          const active = document.activeElement;
          if (active instanceof HTMLElement) active.click();
        }

        if (justPressed(BUTTON_B)) {
          callbacks.current.onBack?.();
        }

        if (justPressed(BUTTON_X)) {
          const gameId = focusedGameId();
          if (gameId) callbacks.current.onPlayGame?.(gameId);
        }

        if (justPressed(BUTTON_START)) {
          callbacks.current.onToggleConsole?.();
        }

        const horizontal = gamepad.axes[0] ?? 0;
        const vertical = gamepad.axes[1] ?? 0;
        let direction: Direction | null = null;

        if (pressed[DPAD_UP] || vertical < -0.65) direction = "up";
        else if (pressed[DPAD_DOWN] || vertical > 0.65) direction = "down";
        else if (pressed[DPAD_LEFT] || horizontal < -0.65) direction = "left";
        else if (pressed[DPAD_RIGHT] || horizontal > 0.65) direction = "right";

        if (direction) {
          const repeat = repeatState.current;
          if (repeat.direction !== direction || timestamp >= repeat.nextAt) {
            moveFocus(direction);
            repeatState.current = {
              direction,
              nextAt: timestamp + (repeat.direction === direction ? 115 : 320),
            };
          }
        } else {
          repeatState.current = { direction: null, nextAt: 0 };
        }

        previousButtons.current = pressed;
      } else if (connectedRef.current) {
        updateConnected(false);
        previousButtons.current = [];
      }

      frame = window.requestAnimationFrame(loop);
    }

    frame = window.requestAnimationFrame(loop);

    return () => {
      window.cancelAnimationFrame(frame);
      window.removeEventListener("gamepadconnected", onConnected);
      window.removeEventListener("gamepaddisconnected", onDisconnected);
    };
  }, [options.enabled]);

  return { connected };
}
