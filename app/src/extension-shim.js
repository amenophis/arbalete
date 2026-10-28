// Injected by Arbalete into every extension's service worker (app/src/extensions.rs). Arbalete's
// pages sit in no Chromium window, so Chrome's tab and window APIs find none: they answer instead
// with the profile's active tab, which the app keeps up to date (__arbaleteSync), and creating or
// updating a tab asks Arbalete (__arbalete, a DevTools binding) to do it in its own tabs.
(() => {
  if (self.__arbaleteSync) return;
  const window_ = {
    id: 1,
    focused: true,
    top: 0,
    left: 0,
    width: 1280,
    height: 820,
    incognito: false,
    type: "normal",
    state: "normal",
    alwaysOnTop: false,
  };
  let tabs = [];
  self.__arbaleteSync = (state) => {
    tabs = state.tabs;
  };
  // Chrome's APIs take a callback last, or return a promise without one.
  const reply = (args, value) => {
    const callback = args.find((arg) => typeof arg === "function");
    if (!callback) return Promise.resolve(value);
    callback(value);
  };
  const ask = (message) => self.__arbalete(JSON.stringify(message));
  // chrome.windows.WINDOW_ID_CURRENT.
  const CURRENT = -2;
  const fits = (tab, info) =>
    (info.active === undefined || tab.active === info.active) &&
    (info.windowId === undefined || info.windowId === CURRENT || info.windowId === tab.windowId);
  const withTabs = (info) =>
    info && typeof info === "object" && info.populate ? { ...window_, tabs } : window_;
  const get = chrome.tabs.get.bind(chrome.tabs);
  Object.assign(chrome.tabs, {
    query: (info = {}, ...rest) => reply(rest, tabs.filter((tab) => fits(tab, info))),
    get: (id, ...rest) => {
      const tab = tabs.find((tab) => tab.id === id);
      return tab ? reply(rest, tab) : get(id, ...rest);
    },
    create: (props = {}, ...rest) => {
      ask({ method: "create", props });
      return reply(rest, { id: -1, index: 0, windowId: window_.id, active: true, url: props.url });
    },
    update: (...args) => {
      const [tabId, props] = typeof args[0] === "number" ? args : [undefined, args[0]];
      ask({ method: "update", tabId, props });
      return reply(args.slice(typeof args[0] === "number" ? 2 : 1), tabs[0]);
    },
  });
  Object.assign(chrome.windows, {
    get: (id, ...rest) => reply(rest, withTabs(rest[0])),
    getCurrent: (...args) => reply(args, withTabs(args[0])),
    getLastFocused: (...args) => reply(args, withTabs(args[0])),
    getAll: (...args) => reply(args, [withTabs(args[0])]),
  });
})();
