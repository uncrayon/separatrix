import { useCallback, useEffect, useReducer, useRef, useState, type KeyboardEvent } from "react";
import { createPortal } from "react-dom";
import { getVersion } from "@tauri-apps/api/app";
import { checkForUpdate, installUpdate, isTauri, listenUpdateEvents, startUpdateCheck } from "../api/client";
import { errorText } from "../project/ipc";
import { initialUpdateState, updateMessage, updateReducer } from "./state";

/** `beforeInstall` resolves true once unsaved work may be dropped (the document's save question). */
export function UpdateButton({ beforeInstall }: { beforeInstall: () => Promise<boolean> }) {
  const [state, dispatch] = useReducer(updateReducer, initialUpdateState);
  const [open, setOpen] = useState(false);
  // The running binary's version, which the update check compares against; the preview has only package.json's.
  const [version, setVersion] = useState(__APP_VERSION__);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);

  const check = useCallback(() => {
    setOpen(true);
    dispatch({ type: "check" });
    checkForUpdate().then(
      (result) => dispatch({ type: "checked", result }),
      (err) => dispatch({ type: "checked", result: { kind: "failed", message: errorText(err) } }),
    );
  }, []);

  useEffect(() => {
    if (isTauri()) void getVersion().then(setVersion, () => {});
  }, []);

  useEffect(() => {
    let off: (() => void) | null = null;
    let gone = false;
    void listenUpdateEvents({
      onAvailable: (result) => dispatch({ type: "checked", result }),
      onCheckRequested: check,
    }).then((f) => {
      if (gone) return f();
      off = f;
      void startUpdateCheck().catch(() => {});
    });
    return () => {
      gone = true;
      off?.();
    };
  }, [check]);

  useEffect(() => {
    if (!open) return;
    const close = (e: PointerEvent) => {
      const t = e.target as Node;
      if (!panelRef.current?.contains(t) && !buttonRef.current?.contains(t)) setOpen(false);
    };
    window.addEventListener("pointerdown", close);
    return () => window.removeEventListener("pointerdown", close);
  }, [open]);

  const install = async () => {
    if (!(await beforeInstall())) return;
    dispatch({ type: "install" });
    installUpdate().catch((err) => dispatch({ type: "installFailed", message: errorText(err) }));
  };

  const onKey = (e: KeyboardEvent) => {
    if (e.key !== "Escape" || !open) return;
    e.preventDefault();
    e.stopPropagation();
    setOpen(false);
    buttonRef.current?.focus();
  };

  const available = state.kind === "available";
  return (
    <span className="ds-statusbar-group" onKeyDown={onKey}>
      <span className="ds-statusbar-sep" aria-hidden="true" />
      <button
        ref={buttonRef}
        type="button"
        className="ds-statusbar-item ds-statusbar-item--button"
        data-tooltip={open ? undefined : "Check for updates"}
        aria-expanded={open}
        aria-controls={open ? "update-panel" : undefined}
        data-update-button=""
        onClick={() => (open && state.kind !== "checking" ? setOpen(false) : check())}
      >
        <span className="ds-statusbar-text">
          <span className="ds-statusbar-num">v{version}</span>
          {available && <span className="ds-statusbar-strong"> · Update available</span>}
        </span>
        <span className="ds-sr">. Check for updates</span>
      </button>
      {open &&
        createPortal(
          <div ref={panelRef} id="update-panel" className="update-panel" role="dialog" aria-label="Updates" onKeyDown={onKey}>
            <p className="update-panel-message body" role="status">
              {updateMessage(state)}
            </p>
            {available && (
              <button type="button" className="ds-button ds-button--primary ds-button--sm" onClick={() => void install()}>
                Install and restart
              </button>
            )}
          </div>,
          document.body,
        )}
    </span>
  );
}
