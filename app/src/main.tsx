import React from "react";
import ReactDOM from "react-dom/client";
import { MathfieldElement } from "mathlive";
// MathLive fonts and static-markup styles, bundled locally so the app works offline.
import "mathlive/fonts.css";
import "mathlive/static.css";
// Separatrix design system (vendored, do not edit): tokens, type styles and
// fonts, then every ds-* component class with the base reset and focus ring.
import "./styles/ds/tokens.css";
import "./styles/ds/components.css";
// App shell layout (grid, panels, plot well, gallery overlay), tokens only.
import "./styles/app.css";
// Per-area app styles, one file per owner, loaded after the shell.
import "./styles/areas/shell.css";
import "./styles/areas/system.css";
import "./styles/areas/inspector.css";
import "./styles/areas/canvas.css";
import App from "./App";

// The fonts come from the CSS imports above; stop MathLive fetching its own copy
// (and its keypress sounds) from a path that does not exist in the bundle.
MathfieldElement.fontsDirectory = null;
MathfieldElement.soundsDirectory = null;

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
