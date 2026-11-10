// Keeps every page of a copy run with ARBALETE_DEBUG_PORT in a color scheme, for screenshots:
// node scheme.mjs <port> <light|dark>. An emulation lasts as long as its connection.
const [port, scheme] = process.argv.slice(2);
const open = new Map();
async function tick() {
  const targets = await (await fetch(`http://localhost:${port}/json`)).json().catch(() => []);
  for (const t of targets) {
    if (t.type !== 'page' || open.has(t.id) || !t.webSocketDebuggerUrl) continue;
    const ws = new WebSocket(t.webSocketDebuggerUrl);
    open.set(t.id, ws);
    ws.onopen = () => ws.send(JSON.stringify({ id: 1, method: 'Emulation.setEmulatedMedia',
      params: { features: [{ name: 'prefers-color-scheme', value: scheme }] } }));
    ws.onclose = () => open.delete(t.id);
  }
}
setInterval(() => tick().catch(() => {}), 700);
