import { html } from "../../vendor/preact-htm.js";

// Renders a refused change in normal document flow, never as a modal, so the
// section that asked for the change is where the refusal reports back.
export function RejectionNotice({ rejection }) {
  if (!rejection) {
    return null;
  }

  return html`
    <div class="rejection-notice">
      <p class="screen-error">
        ${rejection.blockingTasks.length > 0
          ? `Blocked by ${rejection.blockingTasks.join(", ")}`
          : rejection.message}
      </p>
      ${rejection.issues.length > 0
        ? html`
            <ul class="issue-list">
              ${rejection.issues.map(
                (issue, index) => html`
                  <li key=${index}>
                    <span class="issue-code">${issue.code}</span>
                    <span class="issue-message">${issue.message}</span>
                    ${issue.path ? html`<span class="issue-path">${issue.path}</span>` : null}
                  </li>
                `,
              )}
            </ul>
          `
        : null}
      ${rejection.allowed.length > 0
        ? html`<span class="field-allowed">Allowed: ${rejection.allowed.join(", ")}</span>`
        : null}
    </div>
  `;
}
