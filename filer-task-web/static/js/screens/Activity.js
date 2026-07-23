import { html, useEffect, useMemo, useState } from "../../vendor/preact-htm.js";
import { getActivity } from "../api/client.js";
import { Header } from "../components/Header.js";

function formatTime(unixSeconds) {
  return new Date(unixSeconds * 1000).toLocaleString();
}

export function ActivityScreen({ projectName }) {
  const query = useMemo(() => ({ project: projectName, limit: 50 }), [projectName]);
  const [rows, setRows] = useState([]);
  const [error, setError] = useState(null);

  async function load() {
    try {
      const activity = await getActivity(query);
      setRows(activity);
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
      <${Header} title="Activity" onRefresh=${load} />
      ${error
        ? html`<p class="screen-error">Could not load activity: ${error.message}</p>`
        : null}
      ${rows.length === 0
        ? html`<p class="empty-state">No activity recorded yet.</p>`
        : html`
            <table class="activity-table">
              <thead>
                <tr>
                  <th>Time</th>
                  <th>User</th>
                  <th>Action</th>
                  <th>Task</th>
                  <th>Detail</th>
                </tr>
              </thead>
              <tbody>
                ${rows.map(
                  (row) => html`
                    <tr key=${row.id}>
                      <td>${formatTime(row.created_at)}</td>
                      <td>${row.username}</td>
                      <td>${row.action}</td>
                      <td>${row.task_id ? html`<code>${row.task_id}</code>` : ""}</td>
                      <td>${row.detail ?? ""}</td>
                    </tr>
                  `,
                )}
              </tbody>
            </table>
          `}
    </section>
  `;
}
