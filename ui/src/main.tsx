import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { revealOnHover } from "./bridge";
import { Import } from "./Import";
import { Overlay } from "./Overlay";
import "./index.css";

const root = document.getElementById("root");
if (!root) throw new Error("index.html has no #root element");

// The same page serves the sidebar, the sidebar revealed above the page with #reveal, the
// overlay above the page with #overlay and the import assistant above the page with #import.
if (location.hash === "#reveal") revealOnHover(document.documentElement);
const pages = { "#overlay": <Overlay />, "#import": <Import /> };
createRoot(root).render(
  <StrictMode>{pages[location.hash as keyof typeof pages] ?? <App />}</StrictMode>,
);
