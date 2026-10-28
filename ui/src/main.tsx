import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { followDocking, revealOnHover } from "./bridge";
import { Find } from "./Find";
import { Import } from "./Import";
import { Little } from "./Little";
import { trackMenuTargets } from "./menuTargets";
import { Overlay } from "./Overlay";
import { Settings } from "./Settings";
import { Site } from "./Site";
import { Toolbar } from "./Toolbar";
import "./index.css";

const root = document.getElementById("root");
if (!root) throw new Error("index.html has no #root element");

// The same page serves the sidebar, the sidebar revealed above the page with #reveal, the
// overlay above the page with #overlay, the find bar above the page with #find, the import
// assistant above the page with #import, the settings window with #settings and the bar above
// the page in toolbar mode with #toolbar, the site controls' panel with #site, and a Little Arbalete window's bar with #little/<id>.
if (location.hash === "#reveal") revealOnHover(document.documentElement);
if (location.hash === "") followDocking(document.documentElement);
const pages = {
  "#overlay": <Overlay />,
  "#find": <Find />,
  "#import": <Import />,
  "#settings": <Settings />,
  "#toolbar": <Toolbar />,
  "#site": <Site />,
};
if (location.hash === "#reveal") trackMenuTargets(root, "revealed");
if (location.hash === "") trackMenuTargets(root, "docked");
createRoot(root).render(
  <StrictMode>
    {location.hash.startsWith("#little/") ? (
      <Little id={Number(location.hash.slice("#little/".length))} />
    ) : (
      (pages[location.hash as keyof typeof pages] ?? <App />)
    )}
  </StrictMode>,
);
