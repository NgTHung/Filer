import { html } from "../../vendor/preact-htm.js";

// Read-only view over a ShowView the caller already has. Lifecycle actions and
// criteria toggles land in WEB-008 on this same component, so it takes the view
// as a prop instead of fetching, and every caller keeps owning its own refresh.
export function TaskDrawer({ view, onClose }) {
  if (!view) {
    return null;
  }
  const { task, sections, criteria, criteria_heading } = view.detail;

  return html`
    <div class="drawer-overlay" onClick=${onClose}>
      <aside class="drawer" onClick=${(event) => event.stopPropagation()}>
        <header class="drawer-header">
          <span class="drawer-id">${task.qualified_id}</span>
          <span class="drawer-badge">${task.status}</span>
          <span class="drawer-badge">${task.priority}</span>
          <button class="drawer-close" onClick=${onClose} title="Close">×</button>
        </header>
        <h3 class="drawer-title">${task.title}</h3>
        <dl class="drawer-meta">
          <dt>Type</dt>
          <dd>${task.type}</dd>
          <dt>Domain</dt>
          <dd>${task.domain}</dd>
          ${task.milestone ? html`<dt>Milestone</dt><dd>${task.milestone}</dd>` : null}
          ${(task.tags ?? []).length > 0 ? html`<dt>Tags</dt><dd>${task.tags.join(", ")}</dd>` : null}
          <dt>Path</dt>
          <dd class="drawer-path">${task.path}</dd>
        </dl>
        ${sections.map(
          (section, index) => html`
            <section key=${`${index}-${section.heading}`} class="drawer-section">
              <h4>${section.heading}</h4>
              <p>${section.content}</p>
            </section>
          `,
        )}
        <section class="drawer-section">
          <h4>${criteria_heading}</h4>
          ${criteria.length === 0
            ? html`<p class="muted-note">None listed.</p>`
            : html`
                <ul class="criteria-list">
                  ${criteria.map(
                    (item, index) => html`
                      <li key=${`${index}-${item.content_hash}`} class=${item.checked ? "criterion-checked" : ""}>
                        <span class="criterion-marker">${item.checked ? "✓" : "○"}</span>
                        ${item.text}
                      </li>
                    `,
                  )}
                </ul>
              `}
        </section>
        ${view.warnings.length > 0
          ? html`
              <section class="drawer-section">
                <h4>Warnings</h4>
                <ul class="issue-list">
                  ${view.warnings.map((warning, index) => html`<li key=${index}>${warning.message}</li>`)}
                </ul>
              </section>
            `
          : null}
      </aside>
    </div>
  `;
}
