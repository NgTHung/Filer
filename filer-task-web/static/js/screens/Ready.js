import { html, useEffect, useMemo, useState } from "../../vendor/preact-htm.js";
import { projectScoped } from "../api/client.js";
import { Header } from "../components/Header.js";

const PRIORITY_ORDER = { High: 0, Medium: 1, Low: 2 };

function sortRows(rows) {
  return [...rows].sort((a, b) => {
    const priorityDiff = (PRIORITY_ORDER[a.priority] ?? 9) - (PRIORITY_ORDER[b.priority] ?? 9);
    if (priorityDiff !== 0) {
      return priorityDiff;
    }
    return a.id < b.id ? -1 : a.id > b.id ? 1 : 0;
  });
}

export function ReadyScreen({ projectName, onSelectTask }) {
  const api = useMemo(() => projectScoped(projectName), [projectName]);
  const [rows, setRows] = useState([]);
  const [error, setError] = useState(null);
  const [domainFilter, setDomainFilter] = useState(null);
  const [milestoneFilter, setMilestoneFilter] = useState(null);

  async function load() {
    try {
      const ready = await api.getReady();
      setRows(ready);
      setError(null);
    } catch (err) {
      setError(err);
    }
  }

  useEffect(() => {
    load();
  }, [projectName]);

  const domainChips = [...new Set(rows.map((row) => row.domain))];
  const milestoneChips = [...new Set(rows.map((row) => row.milestone).filter(Boolean))];
  const filtered = rows.filter(
    (row) => (!domainFilter || row.domain === domainFilter) && (!milestoneFilter || row.milestone === milestoneFilter),
  );
  const sorted = sortRows(filtered);

  return html`
    <section class="screen">
      <${Header} title="Ready work" onRefresh=${load} />
      ${error
        ? html`<p class="screen-error">Could not load ready work: ${error.message}</p>`
        : null}
      <div class="chip-row">
        ${domainChips.map(
          (domain) => html`
            <button
              class="chip ${domainFilter === domain ? "chip-active" : ""}"
              onClick=${() => setDomainFilter(domainFilter === domain ? null : domain)}
            >
              ${domain}
            </button>
          `,
        )}
        ${milestoneChips.map(
          (milestone) => html`
            <button
              class="chip ${milestoneFilter === milestone ? "chip-active" : ""}"
              onClick=${() => setMilestoneFilter(milestoneFilter === milestone ? null : milestone)}
            >
              ${milestone}
            </button>
          `,
        )}
      </div>
      ${sorted.length === 0
        ? html`<p class="empty-state">${emptyReason(rows, domainFilter || milestoneFilter)}</p>`
        : html`
            <table class="ready-table">
              <thead>
                <tr>
                  <th>Id</th>
                  <th>Title</th>
                  <th>Priority</th>
                  <th>Domain</th>
                  <th>Milestone</th>
                </tr>
              </thead>
              <tbody>
                ${sorted.map(
                  (row) => html`
                    <tr
                      key=${row.qualified_id}
                      class="task-row"
                      onClick=${() => onSelectTask(row.qualified_id)}
                    >
                      <td>${row.qualified_id}</td>
                      <td>${row.title}</td>
                      <td>${row.priority}</td>
                      <td>${row.domain}</td>
                      <td>${row.milestone ?? ""}</td>
                    </tr>
                  `,
                )}
              </tbody>
            </table>
          `}
    </section>
  `;
}

function emptyReason(rows, filtered) {
  if (filtered) {
    return "No ready tasks match the selected filters.";
  }
  if (rows.length === 0) {
    return "Nothing is ready: every To Do task has an unfinished dependency, or there are no To Do tasks.";
  }
  return "No ready tasks.";
}
