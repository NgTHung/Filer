import { html } from "../../vendor/preact-htm.js";
import { loadProjects } from "../store/project.js";

// Renders for a ProjectSummary with broken: true, or after catching a 422
// ProjectBroken response, instead of the Ready screen.
export function BrokenScreen({ project, onSwitchProject }) {
  const issues = project?.issues ?? [];

  return html`
    <section class="screen broken-screen">
      <div class="screen-header">
        <h2>${project ? project.name : "Project"} failed validation</h2>
      </div>
      <p class="screen-error">
        This project's .tasks/ tree does not validate. Fix the issues below, then re-validate.
      </p>
      ${issues.length > 0
        ? html`
            <ul class="issue-list">
              ${issues.map(
                (issue) => html`
                  <li key=${issue.code + (issue.path ?? "")}>
                    <span class="issue-code">${issue.code}</span>
                    <span class="issue-message">${issue.message}</span>
                    ${issue.path ? html`<span class="issue-path">${issue.path}</span>` : null}
                  </li>
                `,
              )}
            </ul>
          `
        : html`<p class="empty-state">No issue detail was returned.</p>`}
      <div class="broken-actions">
        <button onClick=${() => loadProjects()}>Re-validate</button>
        <button onClick=${onSwitchProject}>Switch project</button>
      </div>
    </section>
  `;
}
