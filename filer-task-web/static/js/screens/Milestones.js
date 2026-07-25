import { html, useEffect, useMemo, useState } from "../../vendor/preact-htm.js";
import { projectScoped } from "../api/client.js";
import { Header } from "../components/Header.js";
import { progressPercent, statusGroups } from "../lib/milestones.js";

export function MilestonesScreen({ projectName }) {
  const api = useMemo(() => projectScoped(projectName), [projectName]);
  const [rows, setRows] = useState([]);
  const [error, setError] = useState(null);

  async function load() {
    try {
      setRows(await api.getMilestones());
      setError(null);
    } catch (err) {
      setError(err);
    }
  }

  useEffect(() => {
    load();
  }, [projectName]);

  return html`
    <section class="screen">
      <${Header} title="Milestones" onRefresh=${load} />
      ${error ? html`<p class="screen-error">Could not load milestones: ${error.message}</p>` : null}
      ${rows.length === 0 && !error
        ? html`<p class="empty-state">No milestone-role tasks are declared in this project.</p>`
        : null}
      ${rows.map((row) => html`<${MilestoneCard} key=${row.milestone.qualified_id} aggregation=${row} />`)}
    </section>
  `;
}

function MilestoneCard({ aggregation }) {
  const { milestone, criteria, criteria_heading, done, total } = aggregation;
  const percent = progressPercent(done, total);

  return html`
    <article class="milestone-card">
      <header class="milestone-card-header">
        <h3>${milestone.milestone ?? milestone.id}</h3>
        <span class="milestone-title">${milestone.title}</span>
        <span class="milestone-status">${milestone.status}</span>
      </header>
      <div class="milestone-progress" role="progressbar" aria-valuenow=${percent} aria-valuemin="0" aria-valuemax="100">
        <div class="milestone-progress-fill" style=${`width: ${percent}%`}></div>
      </div>
      <p class="milestone-progress-label">${done} of ${total} done (${percent}%)</p>
      <h4>${criteria_heading}</h4>
      ${criteria.length === 0
        ? html`<p class="milestone-empty">No ${criteria_heading.toLowerCase()} listed.</p>`
        : html`
            <ul class="milestone-criteria">
              ${criteria.map(
                (item, index) => html`
                  <li key=${index} class=${item.checked ? "criterion-checked" : ""}>
                    <span class="criterion-marker">${item.checked ? "✓" : "○"}</span>
                    ${item.text}
                  </li>
                `,
              )}
            </ul>
          `}
      ${statusGroups(aggregation.tasks_by_status).map(
        ([status, tasks]) => html`
          <div key=${status} class="milestone-group">
            <h4>${status} <span class="milestone-group-count">${tasks.length}</span></h4>
            <ul class="milestone-task-list">
              ${tasks.map(
                (task) => html`
                  <li key=${task.qualified_id}>
                    <span class="milestone-task-id">${task.qualified_id}</span>
                    <span class="milestone-task-title">${task.title}</span>
                  </li>
                `,
              )}
            </ul>
          </div>
        `,
      )}
    </article>
  `;
}
