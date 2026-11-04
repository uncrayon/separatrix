import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

interface Shown {
  label: string;
  desc?: string;
  rect: DOMRect;
  instant: boolean;
}

const HOVER_DELAY = 500;
const GRACE = 300;
const GAP = 8;

export function TooltipLayer() {
  const [tip, setTip] = useState<Shown | null>(null);
  const boxRef = useRef<HTMLSpanElement>(null);
  const [pos, setPos] = useState<{ left: number; top: number } | null>(null);

  useEffect(() => {
    let timer = 0;
    let current: HTMLElement | null = null;
    let lastClose = -Infinity;

    const find = (t: EventTarget | null) =>
      t instanceof Element ? t.closest<HTMLElement>("[data-tooltip]") : null;
    const show = (el: HTMLElement, instant: boolean) => {
      window.clearTimeout(timer);
      current = el;
      setPos(null);
      setTip({
        label: el.dataset.tooltip ?? "",
        desc: el.dataset.tooltipDesc || undefined,
        rect: el.getBoundingClientRect(),
        instant,
      });
    };
    const hide = () => {
      window.clearTimeout(timer);
      if (current) lastClose = performance.now();
      current = null;
      setTip(null);
    };

    const onOver = (e: PointerEvent) => {
      if (e.pointerType === "touch") return;
      const el = find(e.target);
      if (el === current) return;
      const wasOpen = current !== null;
      hide();
      if (!el || !el.dataset.tooltip) return;
      if (wasOpen || performance.now() - lastClose < GRACE) show(el, true);
      else timer = window.setTimeout(() => show(el, false), HOVER_DELAY);
    };
    const onLeaveWindow = () => hide();
    const onFocusIn = (e: FocusEvent) => {
      const el = find(e.target);
      if (el && el === e.target && el.dataset.tooltip && el.matches(":focus-visible")) show(el, true);
    };
    const onFocusOut = (e: FocusEvent) => {
      if (current && e.target === current) hide();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && current) {
        if (current === document.activeElement) e.stopPropagation();
        hide();
      }
    };

    document.addEventListener("pointerover", onOver);
    document.documentElement.addEventListener("pointerleave", onLeaveWindow);
    document.addEventListener("pointerdown", hide, true);
    document.addEventListener("scroll", hide, true);
    document.addEventListener("focusin", onFocusIn);
    document.addEventListener("focusout", onFocusOut);
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("resize", hide);
    return () => {
      window.clearTimeout(timer);
      document.removeEventListener("pointerover", onOver);
      document.documentElement.removeEventListener("pointerleave", onLeaveWindow);
      document.removeEventListener("pointerdown", hide, true);
      document.removeEventListener("scroll", hide, true);
      document.removeEventListener("focusin", onFocusIn);
      document.removeEventListener("focusout", onFocusOut);
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("resize", hide);
    };
  }, []);

  useLayoutEffect(() => {
    const box = boxRef.current;
    if (!tip || !box) return;
    const w = box.offsetWidth;
    const h = box.offsetHeight;
    const r = tip.rect;
    const vw = window.innerWidth;
    const vh = window.innerHeight;
    const above = r.top + r.height / 2 > vh / 2;
    let top = above ? r.top - GAP - h : r.bottom + GAP;
    top = Math.max(GAP, Math.min(vh - GAP - h, top));
    let left = r.left;
    left = Math.max(GAP, Math.min(vw - GAP - w, left));
    if (!pos || pos.left !== left || pos.top !== top) setPos({ left, top });
  }, [tip, pos]);

  if (!tip) return null;
  return createPortal(
    <div className="tooltip-layer" aria-hidden="true">
      <span
        ref={boxRef}
        className={"ds-tooltip" + (tip.instant ? " is-instant" : "")}
        role="tooltip"
        style={pos ? { left: pos.left, top: pos.top } : { left: 0, top: 0, visibility: "hidden" }}
      >
        <span className="ds-tooltip-label label">{tip.label}</span>
        {tip.desc && <span className="ds-tooltip-desc caption">{tip.desc}</span>}
      </span>
    </div>,
    document.body,
  );
}
