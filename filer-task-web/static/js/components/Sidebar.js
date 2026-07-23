import { html, useEffect, useState } from "../../vendor/preact-htm.js";
import { projectScoped } from "../api/client.js";
import { activeProject, setActiveProject, useProjectStore } from "../store/project.js";
import { IdentityFooter } from "./IdentityFooter.js";

const NAV_ITEMS = [
  { id: "ready", label: "Ready" },
  { id: "tasks", label: "Tasks" },
  { id: "milestones", label: "Milestones" },
  { id: "new-task", label: "New task" },
  { id: "activity", label: "Activity" },
];

function summarize(tasks) {
  const domains = new Map();
  for (const task of tasks) {
    const entry = domains.get(task.domain) ?? { count: 0, blocked: false };
    entry.count += 1;
    if (task.status === "Blocked") {
      entry.blocked = true;
    }
    domains.set(task.domain, entry);
  }
  return domains;
}

export function Sidebar({ screen, onSelectScreen, onSwitchProject }) {
  const store = useProjectStore();
  const project = activeProject();
  const [tasks, setTasks] = useState([]);
  const [readyCount, setReadyCount] = useState(0);
  const [milestoneCount, setMilestoneCount] = useState(0);

  useEffect(() => {
    if (!project || project.broken) {
      setTasks([]);
      setReadyCount(0);
      setMilestoneCount(0);
      return;
    }
    const api = projectScoped(project.name);
    let cancelled = false;
    Promise.all([api.getTasks(), api.getReady(), api.getMilestones()])
      .then(([allTasks, ready, milestones]) => {
        if (cancelled) {
          return;
        }
        setTasks(allTasks);
        setReadyCount(ready.length);
        setMilestoneCount(milestones.length);
      })
      .catch(() => {
        if (!cancelled) {
          setTasks([]);
          setReadyCount(0);
          setMilestoneCount(0);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [project?.name]);

  const domains = summarize(tasks);
  const navCounts = {
    ready: readyCount,
    tasks: tasks.length,
    milestones: milestoneCount,
    "new-task": null,
    activity: null,
  };

  return html`
    <aside class="sidebar">
      <button class="project-switcher" onClick=${onSwitchProject} title="Switch project">
        <span class="project-name">${project ? project.name : "No project"}</span>
        <span class="switcher-caret">⌄</span>
      </button>
      <nav class="nav-list">
        ${NAV_ITEMS.map(
          (item) => html`
            <button
              class="nav-item ${screen === item.id ? "active" : ""}"
              onClick=${() => onSelectScreen(item.id)}
            >
              <span>${item.label}</span>
              ${navCounts[item.id] !== null ? html`<span class="nav-count">${navCounts[item.id]}</span>` : null}
            </button>
          `,
        )}
      </nav>
      <div class="domain-list">
        <h3>Domains</h3>
        ${[...domains.entries()].map(
          ([name, info]) => html`
            <div class="domain-row">
              <span class="domain-name">${info.blocked ? html`<span class="blocked-dot"></span>` : null}${name}</span>
              <span class="domain-count">${info.count}</span>
            </div>
          `,
        )}
        ${domains.size === 0 ? html`<p class="domain-empty">No domains yet.</p>` : null}
      </div>
      <${IdentityFooter} />
    </aside>
  `;
}
