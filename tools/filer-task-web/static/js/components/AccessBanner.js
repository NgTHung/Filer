import { html, useState } from "../../vendor/preact-htm.js";
import { projectScoped } from "../api/client.js";
import { accessBanner } from "../lib/access.js";
import { loadProjects } from "../store/project.js";

// Shown above a project's screens when the user cannot change it. Reloading
// the project list after a request or withdrawal refreshes my_role, which is
// what decides whether this banner shows at all.
export function AccessBanner({ project }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(null);
  const state = accessBanner(project.my_role);
  if (!state) {
    return null;
  }

  async function run(change) {
    setBusy(true);
    setError(null);
    try {
      await change(projectScoped(project.name));
      await loadProjects();
    } catch (requestError) {
      setError(requestError);
    } finally {
      setBusy(false);
    }
  }

  return html`
    <div class="access-banner" role="status">
      ${state === "pending"
        ? html`
            <span>Access requested</span>
            <button
              type="button"
              disabled=${busy}
              onClick=${() => run((api) => api.withdrawAccessRequest())}
            >
              Withdraw
            </button>
          `
        : html`
            <span>You can view this project. Request access to make changes.</span>
            <button type="button" disabled=${busy} onClick=${() => run((api) => api.requestAccess())}>
              Request access
            </button>
          `}
      ${error ? html`<span class="access-banner-error">${error.message}</span>` : null}
    </div>
  `;
}
