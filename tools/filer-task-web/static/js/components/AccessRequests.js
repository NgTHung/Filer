import { html, useEffect, useMemo, useState } from "../../vendor/preact-htm.js";
import { projectScoped } from "../api/client.js";

function formatTime(unixSeconds) {
  return new Date(unixSeconds * 1000).toLocaleString();
}

// Pending requests for one project, shown to owners. Approval grants the
// editor role; an owner promotes an editor from the member list afterwards.
export function AccessRequests({ projectName, reloadKey, onApproved }) {
  const api = useMemo(() => projectScoped(projectName), [projectName]);
  const [requests, setRequests] = useState([]);
  const [error, setError] = useState(null);
  const [busyId, setBusyId] = useState(null);

  async function load() {
    try {
      const response = await api.listAccessRequests();
      setRequests(response.requests);
      setError(null);
    } catch (requestError) {
      setError(requestError);
    }
  }

  useEffect(() => {
    load();
  }, [projectName, reloadKey]);

  async function run(request, change) {
    setBusyId(request.user_id);
    setError(null);
    try {
      await change();
      await load();
    } catch (requestError) {
      setError(requestError);
    } finally {
      setBusyId(null);
    }
  }

  function approve(request) {
    run(request, async () => {
      await api.setMemberRole(request.user_id, "editor");
      onApproved();
    });
  }

  return html`
    <div class="member-list">
      ${error
        ? html`<p class="screen-error" role="alert">Access request failed: ${error.message}</p>`
        : null}
      ${requests.length === 0 && !error
        ? html`<p class="empty-state">No pending requests.</p>`
        : null}
      ${requests.map(
        (request) => html`
          <div class="session-row" key=${request.user_id}>
            <div class="session-label">
              <span>${request.username}</span>
            </div>
            <div class="session-meta">
              <span>Requested ${formatTime(request.requested_at)}</span>
            </div>
            <button type="button" disabled=${busyId === request.user_id} onClick=${() => approve(request)}>
              Approve
            </button>
            <button
              type="button"
              class="session-revoke"
              disabled=${busyId === request.user_id}
              onClick=${() => run(request, () => api.denyAccessRequest(request.user_id))}
            >
              Deny
            </button>
          </div>
        `,
      )}
    </div>
  `;
}
