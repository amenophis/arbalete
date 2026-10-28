import { send } from "./bridge";

// An extension the user installed and enabled in the displayed Space's profile (installed in
// app/src/extensions.rs).
export type Extension = {
  id: string;
  name: string;
  icon: string | null;
  // Its button opens a popup, rather than telling the extension it was clicked.
  popup: boolean;
};

// The UI views a button may show in, where the app finds it to open its popup under it.
export type ButtonView = "docked" | "revealed" | "toolbar";

// The buttons of the extensions that have a popup, which a click opens under the button, in a
// panel above the page, like Chrome's (open_extension_popup in app/src/shell.rs).
export function ExtensionButtons({
  extensions = [],
  view,
}: {
  extensions?: Extension[];
  view: ButtonView;
}) {
  return extensions
    .filter((extension) => extension.popup)
    .map((extension) => (
      <button
        key={extension.id}
        type="button"
        className="extension-button"
        aria-label={extension.name}
        title={extension.name}
        onClick={(event) => {
          const { x, y, width, height } = event.currentTarget.getBoundingClientRect();
          const rect = [x, y, width, height].map(Math.round).join("/");
          send(`extension/popup/${extension.id}/${view}/${rect}`);
        }}
      >
        {extension.icon ? <img src={extension.icon} alt="" /> : extension.name.slice(0, 1)}
      </button>
    ));
}
