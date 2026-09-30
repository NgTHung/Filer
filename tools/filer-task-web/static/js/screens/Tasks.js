import { html, useEffect, useMemo, useState } from "../../vendor/preact-htm.js";
import { projectScoped } from "../api/client.js";
import { Header } from "../components/Header.js";
import { FilterMenu } from "../components/FilterMenu.js";
import { activeFilterEntries, hasActiveFilters, rejectionFor, withoutField } from "../lib/filters.js";
import { SERVER_SORT_KEY, sortRows } from "../lib/sorting.js";

const COLUMNS = [
  { key: "id", label: "Id" },
  { key: "title", label: "Title" },
  { key: "status", label: "Status" },
  { key: "priority", label: "Priority" },
  { key: "type", label: "Type" },
  { key: "milestone", label: "Milestone" },
  { key: "updated", label: "Updated" },
];

export function TasksScreen({ projectName, onSelectTask }) {
  const api = useMemo(() => projectScoped(projectName), [projectName]);
  const [rows, setRows] = useState([]);
  const [policy, setPolicy] = useState(null);
  const [applied, setApplied] = useState({});
  const [rejected, setRejected] = useState([]);
  const [sortKey, setSortKey] = useState("id");
  const [sortDir, setSortDir] = useState("asc");
  const [error, setError] = useState(null);

  // Each rejected filter is dropped and the query retried, so one request can
  // strip several values; the loop bounds itself by the fields it can drop.
  async function fetchRows(filters, sortColumn) {
    let query = { ...filters };
    const rejections = [];
    for (let attempt = 0; attempt <= activeFilterEntries(filters).length; attempt += 1) {
      try {
        const result = await api.getTasks({ ...query, sort_by: SERVER_SORT_KEY[sortColumn] });
        setRows(result);
        setApplied(query);
        setRejected(rejections);
        setError(null);
        return;
      } catch (err) {
        const rejection = rejectionFor(err, query);
        if (!rejection) {
          setRows([]);
          setRejected(rejections);
          setError(err);
          return;
        }
        rejections.push(rejection);
        query = withoutField(query, rejection.field);
      }
    }
  }

  async function loadPolicy() {
    try {
      setPolicy(await api.getPolicy());
    } catch (err) {
      setPolicy(null);
    }
  }

  useEffect(() => {
    loadPolicy();
    fetchRows({}, sortKey);
  }, [projectName]);

  function applyFilters(filters) {
    fetchRows(filters, sortKey);
  }

  function clearFilters() {
    setRejected([]);
    fetchRows({}, sortKey);
  }

  function toggleSort(column) {
    const nextDir = sortKey === column && sortDir === "asc" ? "desc" : "asc";
    setSortKey(column);
    setSortDir(nextDir);
    fetchRows(applied, column);
  }

  function refresh() {
    return fetchRows(applied, sortKey);
  }

  const sorted = sortRows(rows, sortKey, sortDir);
  const filtered = hasActiveFilters(applied) || rejected.length > 0;

  return html`
    <section class="screen">
      <${Header} title="Tasks" onRefresh=${refresh} />
      ${error ? html`<p class="screen-error">Could not load tasks: ${error.message}</p>` : null}
      <${FilterMenu}
        policy=${policy}
        applied=${applied}
        rejected=${rejected}
        onApply=${applyFilters}
        onClear=${clearFilters}
      />
      ${sorted.length === 0
        ? html`
            <div class="empty-state">
              <p>${filtered ? "No tasks match the selected filters." : "No tasks yet."}</p>
              ${filtered ? html`<button onClick=${clearFilters}>Clear filters</button>` : null}
            </div>
          `
        : html`
            <table class="tasks-table">
              <thead>
                <tr>
                  ${COLUMNS.map(
                    (column) => html`
                      <th key=${column.key}>
                        <button class="sort-header" onClick=${() => toggleSort(column.key)}>
                          ${column.label}
                          ${sortKey === column.key ? html`<span class="sort-arrow">${sortDir === "asc" ? "▲" : "▼"}</span>` : null}
                        </button>
                      </th>
                    `,
                  )}
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
                      <td>${row.status}</td>
                      <td>${row.priority}</td>
                      <td>${row.type}</td>
                      <td>${row.milestone ?? ""}</td>
                      <td>${row.last_updated ?? ""}</td>
                    </tr>
                  `,
                )}
              </tbody>
            </table>
          `}
    </section>
  `;
}
