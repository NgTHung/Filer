import { html, render, useEffect, useState } from "../vendor/preact-htm.js";
import { activeProject, loadProjects, useProjectStore } from "./store/project.js";
import { Sidebar } from "./components/Sidebar.js";
import { IdentityPrompt } from "./components/IdentityPrompt.js";
import { ProjectSwitcher } from "./components/ProjectSwitcher.js";
import { ReadyScreen } from "./screens/Ready.js";
import { BrokenScreen } from "./screens/Broken.js";
import { ActivityScreen } from "./screens/Activity.js";
import { TasksScreen } from "./screens/Tasks.js";
import { MilestonesScreen } from "./screens/Milestones.js";
import { NewTaskScreen } from "./screens/NewTask.js";
import { TaskDrawer } from "./components/TaskDrawer.js";
import { loadIdentity, useIdentityStore } from "./store/identity.js";

function Screen({ screen, project, onCreated }) {
  if (screen === "ready") {
    return html`<${ReadyScreen} projectName=${project.name} />`;
  }
  if (screen === "activity") {
    return html`<${ActivityScreen} projectName=${project.name} />`;
  }
  if (screen === "tasks") {
    return html`<${TasksScreen} projectName=${project.name} />`;
  }
  if (screen === "milestones") {
    return html`<${MilestonesScreen} projectName=${project.name} />`;
  }
  return html`<${NewTaskScreen} projectName=${project.name} onCreated=${onCreated} />`;
}

function App() {
  const identityStore = useIdentityStore();
  const store = useProjectStore();
  const [screen, setScreen] = useState("ready");
  const [switcherOpen, setSwitcherOpen] = useState(false);
  const [drawerView, setDrawerView] = useState(null);
  const project = activeProject();

  // A drawer left open across a project switch would show the old project's
  // task while the shell has already moved to the new one.
  useEffect(() => {
    setDrawerView(null);
  }, [project?.name]);

  if (identityStore.loading) {
    return html`<div class="app-loading">Loading identity…</div>`;
  }

  if (identityStore.error) {
    return html`<div class="app-loading">Could not load identity: ${identityStore.error.message}</div>`;
  }

  if (!identityStore.identity) {
    return html`<${IdentityPrompt} onComplete=${loadProjects} />`;
  }

  if (store.loading && store.projects.length === 0) {
    return html`<div class="app-loading">Loading projects…</div>`;
  }

  if (store.error) {
    return html`<div class="app-loading">Could not load projects: ${store.error.message}</div>`;
  }

  if (!project) {
    return html`<div class="app-loading">No projects registered.</div>`;
  }

  return html`
    <div class="app-shell">
      <${Sidebar}
        screen=${screen}
        onSelectScreen=${setScreen}
        onSwitchProject=${() => setSwitcherOpen(true)}
      />
      <main class="app-main">
        ${project.broken
          ? html`<${BrokenScreen} project=${project} onSwitchProject=${() => setSwitcherOpen(true)} />`
          : html`<${Screen} screen=${screen} project=${project} onCreated=${setDrawerView} />`}
      </main>
      <${TaskDrawer} view=${drawerView} onClose=${() => setDrawerView(null)} />
      ${switcherOpen ? html`<${ProjectSwitcher} onClose=${() => setSwitcherOpen(false)} />` : null}
    </div>
  `;
}

loadIdentity().then((identity) => {
  if (identity) {
    loadProjects();
  }
});
render(html`<${App} />`, document.getElementById("app"));
