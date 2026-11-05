// The export dialog over the studio's real state, in the browser preview
// (mocks), for export.e2e.cjs. Served by the dev server only; the app
// itself opens the dialog from the top bar.

import { useEffect, useRef, useState } from "react";
import ReactDOM from "react-dom/client";
import { MathfieldElement } from "mathlive";
import "mathlive/fonts.css";
import "mathlive/static.css";
import "../styles/ds/tokens.css";
import "../styles/ds/components.css";
import "../styles/app.css";
import { ExportDialog } from "../export/ExportDialog";
import { useStudio } from "../state/useStudio";
import { templateById } from "../templates";

MathfieldElement.fontsDirectory = null;
MathfieldElement.soundsDirectory = null;

const params = new URLSearchParams(location.search);
document.documentElement.dataset.theme = params.get("theme") === "light" ? "light" : "dark";

declare global {
  interface Window {
    __exportHarness?: { settings: Record<string, unknown>[]; setEquation: (i: number, source: string) => void };
  }
}

function Harness() {
  const { state, actions } = useStudio();
  const [open, setOpen] = useState(false);
  const settings = useRef<Record<string, unknown>[]>([]);
  useEffect(() => {
    const t = templateById(params.get("template") ?? "rabbits-vs-sheep");
    if (t) actions.loadTemplate(t);
  }, [actions]);
  useEffect(() => {
    window.__exportHarness = {
      settings: settings.current,
      setEquation: (i, source) => actions.setEquation(state.equations[i].id, source),
    };
  });
  return (
    <div className="ds-root" style={{ padding: 16 }}>
      <button type="button" className="ds-button" data-harness-open disabled={!state.compiled} onClick={() => setOpen(true)}>
        Export…
      </button>
      <span data-harness-ready={state.compiled ? "yes" : "no"} />
      {open && (
        <ExportDialog
          state={state}
          settings={settings.current[settings.current.length - 1]}
          onSettingsChange={(o) => settings.current.push(o)}
          onClose={() => setOpen(false)}
        />
      )}
    </div>
  );
}

ReactDOM.createRoot(document.getElementById("root")!).render(<Harness />);
