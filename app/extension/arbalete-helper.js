// Arbalete's helper extension, in every profile (app/src/extensions.rs): extensions know a page by
// its tab id, which only an extension holding the `debugger` permission can match with the page's
// DevTools target. The app calls this from its DevTools session with the worker.
self.arbaleteTargets = () =>
  new Promise((resolve) => chrome.debugger.getTargets(resolve)).then((targets) =>
    targets.filter((target) => target.tabId !== undefined).map(({ id, tabId }) => ({ id, tabId })),
  );

// A service worker starts only for an event it listens to: these start it with each profile, and
// the app's DevTools session, once attached, keeps it running.
chrome.runtime.onStartup.addListener(() => {});
chrome.runtime.onInstalled.addListener(() => {});
