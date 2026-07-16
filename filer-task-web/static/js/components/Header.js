import { html, useState } from "../../vendor/preact-htm.js";

// Shared screen header mounted by every screen: title, loaded-at label, refresh.
export function Header({ title, onRefresh }) {
  const [loadedAt, setLoadedAt] = useState(new Date());

  async function refresh() {
    await onRefresh();
    setLoadedAt(new Date());
  }

  return html`
    <div class="screen-header">
      <h2>${title}</h2>
      <div class="screen-header-meta">
        <span class="loaded-at">Loaded ${loadedAt.toLocaleTimeString()}</span>
        <button onClick=${refresh}>Refresh</button>
      </div>
    </div>
  `;
}
