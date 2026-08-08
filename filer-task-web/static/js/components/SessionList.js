import { html, useState } from "../../vendor/preact-htm.js";
import { revokeSession } from "../api/client.js";

function formatTime(unixSeconds) {
  return new Date(unixSeconds * 1000).toLocaleString();
}

export function SessionList({ sessions, error, onRevoked, onError, busy }) {
  const [busyId, setBusyId] = useState(null);
  const rows = sessions ?? [];
  const errorMessage = error ? `Session request failed: ${error.message}` : null;

  async function revoke(id, deviceLabel) {
    if (!confirm(`Revoke ${deviceLabel}? This browser will lose access on its next request.`)) {
      return;
    }
    setBusyId(id);
    onError(null);
    try {
      await revokeSession(id);
      await onRevoked();
    } catch (requestError) {
      onError(requestError);
    } finally {
      setBusyId(null);
    }
  }

  return html`
    <div class="session-list">
      ${error
        ? html`
            <p class="screen-error" role="alert">
              ${errorMessage}
            </p>
          `
        : null}
      ${busy && rows.length === 0 && !error
        ? html`<p class="muted-note">Loading sessions…</p>`
        : rows.length === 0 && !error
          ? html`<p class="empty-state">No active sessions.</p>`
          : rows.map(
              (session) => html`
                <div class="session-row" key=${session.id}>
                  <div class="session-label">
                    <span>${session.device_label}</span>
                    ${session.current
                      ? html`<span class="session-current">Current</span>`
                      : null}
                  </div>
                  <div class="session-meta">
                    <span>Created ${formatTime(session.created_at)}</span>
                    <span>Last seen ${formatTime(session.last_seen)}</span>
                  </div>
                  ${session.current
                    ? null
                    : html`
                        <button
                          type="button"
                          class="session-revoke"
                          disabled=${busy || busyId === session.id}
                          onClick=${() => revoke(session.id, session.device_label)}
                        >
                          ${busyId === session.id ? "Revoking…" : "Revoke"}
                        </button>
                      `}
                </div>
              `,
            )}
    </div>
  `;
}
