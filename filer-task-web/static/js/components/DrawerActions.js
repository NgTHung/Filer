import { html, useState } from "../../vendor/preact-htm.js";
import { REASON_ACTIONS } from "../lib/taskDetail.js";

const ACTIONS = [
  { action: "start", label: "Start" },
  { action: "done", label: "Done" },
  { action: "block", label: "Block" },
  { action: "defer", label: "Defer" },
  { action: "obsolete", label: "Obsolete" },
];

// Lifecycle bar for the drawer. Block, defer, and obsolete collect their reason
// inline instead of through window.prompt, so the reason is editable and the
// pending transition is visible while it is typed.
export function DrawerActions({ pendingAction, onRun }) {
  const [reasonFor, setReasonFor] = useState(null);
  const [reason, setReason] = useState("");

  function choose(action) {
    if (REASON_ACTIONS.includes(action)) {
      setReasonFor(reasonFor === action ? null : action);
      setReason("");
      return;
    }
    onRun(action);
  }

  function confirm(event) {
    event.preventDefault();
    const pending = reasonFor;
    setReasonFor(null);
    setReason("");
    onRun(pending, reason.trim());
  }

  function cancel() {
    setReasonFor(null);
    setReason("");
  }

  return html`
    <div class="drawer-actions">
      ${ACTIONS.map(
        ({ action, label }) => html`
          <button
            key=${action}
            type="button"
            class="drawer-action ${reasonFor === action ? "drawer-action-open" : ""}"
            disabled=${pendingAction !== null}
            onClick=${() => choose(action)}
          >
            ${pendingAction === action ? "Working…" : label}
          </button>
        `,
      )}
    </div>
    ${reasonFor
      ? html`
          <form class="drawer-reason" onSubmit=${confirm}>
            <label>
              Reason for ${reasonFor}
              <input
                type="text"
                value=${reason}
                autofocus
                onInput=${(event) => setReason(event.currentTarget.value)}
              />
            </label>
            <div class="drawer-reason-actions">
              <button type="submit" disabled=${reason.trim() === ""}>Confirm ${reasonFor}</button>
              <button type="button" onClick=${cancel}>Cancel</button>
            </div>
          </form>
        `
      : null}
  `;
}
