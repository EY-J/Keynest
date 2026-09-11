import { useEffect } from "react";

const SCROLLING_CLASS = "is-scrolling";
const DEFAULT_IDLE_DELAY_MS = 800;

function scrollContainer(target: EventTarget | null): Element | null {
  if (target instanceof Document) {
    return target.scrollingElement;
  }

  return target instanceof Element ? target : null;
}

export function useScrollActivity(
  idleDelayMs = DEFAULT_IDLE_DELAY_MS,
) {
  useEffect(() => {
    const idleTimers = new Map<Element, number>();

    function handleScroll(event: Event) {
      const container = scrollContainer(event.target);
      if (!container) return;

      container.classList.add(SCROLLING_CLASS);

      const existingTimer = idleTimers.get(container);
      if (existingTimer !== undefined) window.clearTimeout(existingTimer);

      idleTimers.set(
        container,
        window.setTimeout(() => {
          container.classList.remove(SCROLLING_CLASS);
          idleTimers.delete(container);
        }, idleDelayMs),
      );
    }

    document.addEventListener("scroll", handleScroll, {
      capture: true,
      passive: true,
    });

    return () => {
      document.removeEventListener("scroll", handleScroll, true);
      for (const [container, timer] of idleTimers) {
        window.clearTimeout(timer);
        container.classList.remove(SCROLLING_CLASS);
      }
      idleTimers.clear();
    };
  }, [idleDelayMs]);
}
