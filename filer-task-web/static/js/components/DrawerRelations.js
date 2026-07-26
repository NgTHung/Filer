import { html } from "../../vendor/preact-htm.js";
import { blockedByChain, relationGroups } from "../lib/taskDetail.js";

// Graph navigation for the drawer: every chip reselects the drawer, so a task's
// neighbourhood is reachable without going back to a list screen.
export function DrawerRelations({ context, onSelect }) {
  const groups = relationGroups(context);
  const blockedBy = blockedByChain(context);

  if (groups.length === 0 && blockedBy.length === 0) {
    return null;
  }

  return html`
    ${groups.map(
      (group) => html`
        <section key=${group.heading} class="drawer-section">
          <h4>${group.heading}</h4>
          <div class="chip-row">
            ${group.chips.map(
              (chip) => html`
                <button
                  key=${chip.qualified_id}
                  type="button"
                  class="relation-chip"
                  title=${chip.title}
                  onClick=${() => onSelect(chip.qualified_id)}
                >
                  <span class="relation-chip-id">${chip.qualified_id}</span>
                  <span class="relation-chip-status">${chip.status}</span>
                </button>
              `,
            )}
          </div>
        </section>
      `,
    )}
    ${blockedBy.length > 0
      ? html`
          <section class="drawer-section">
            <h4>Blocked by</h4>
            <div class="chip-row">
              ${blockedBy.map(
                (blocker) => html`
                  <button
                    key=${blocker.qualified_id}
                    type="button"
                    class="relation-chip relation-chip-blocking"
                    onClick=${() => onSelect(blocker.qualified_id)}
                  >
                    <span class="relation-chip-id">${blocker.qualified_id}</span>
                    <span class="relation-chip-status">${blocker.status ?? blocker.kind}</span>
                  </button>
                `,
              )}
            </div>
          </section>
        `
      : null}
  `;
}
