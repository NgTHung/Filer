import { html, render, useEffect, useState } from "../vendor/preact-htm.js";
import { activeProject, loadProjects, useProjectStore } from "./store/project.js";
import { Sidebar } from "./components/Sidebar.js";
import { IdentityPrompt } from "./components/IdentityPrompt.js";
import { CommandPalette } from "./components/CommandPalette.js";
import { ReadyScreen } from "./screens/Ready.js";
import { BrokenScreen } from "./screens/Broken.js";
import { ActivityScreen } from "./screens/Activity.js";
import { TasksScreen } from "./screens/Tasks.js";
import { MilestonesScreen } from "./screens/Milestones.js";
import { NewTaskScreen } from "./screens/NewTask.js";
import { TaskDrawer } from "./components/TaskDrawer.js";
import { isPaletteShortcut } from "./lib/palette.js";
import { loadIdentity, useIdentityStore } from "./store/identity.js";

function Screen({ screen, project, onSelectTask }) {
  if (screen === "ready") {
    return html`<${ReadyScreen} projectName=${project.name} onSelectTask=${onSelectTask} />`;
  }
  if (screen === "activity") {
    return html`<${ActivityScreen} projectName=${project.name} onSelectTask=${onSelectTask} />`;
  }
  if (screen === "tasks") {
    return html`<${TasksScreen} projectName=${project.name} onSelectTask=${onSelectTask} />`;
  }
  if (screen === "milestones") {
    return html`<${MilestonesScreen} projectName=${project.name} onSelectTask=${onSelectTask} />`;
  }
  return html`<${NewTaskScreen} projectName=${project.name} onCreated=${onSelectTask} />`;
}

function App() {
  const identityStore = useIdentityStore();
  const store = useProjectStore();
  const [screen, setScreen] = useState("ready");
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [selectedTaskId, setSelectedTaskId] = useState(null);
  const project = activeProject();

  // A drawer left open across a project switch would show the old project's
  // task while the shell has already moved to the new one.
  useEffect(() => {
    setSelectedTaskId(null);
  }, [project?.name]);

  // Loading from inside the component, after useIdentityStore and
  // useProjectStore have registered their listeners: a module-level load can
  // resolve before those effects run, and the store notification it fires is
  // then delivered to nobody, stranding the app on "Loading identity…".
  useEffect(() => {
    loadIdentity().then((identity) => {
      if (identity) {
        loadProjects();
      }
    });
  }, []);

  useEffect(() => {
    function onKeyDown(event) {
      if (isPaletteShortcut(event)) {
        event.preventDefault();
        setPaletteOpen((open) => !open);
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

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
        onSwitchProject=${() => setPaletteOpen(true)}
      />
      <main class="app-main">
        ${project.broken
          ? html`<${BrokenScreen} project=${project} onSwitchProject=${() => setPaletteOpen(true)} />`
          : html`<${Screen} screen=${screen} project=${project} onSelectTask=${setSelectedTaskId} />`}
      </main>
      <${TaskDrawer}
        projectName=${project.name}
        taskId=${selectedTaskId}
        onClose=${() => setSelectedTaskId(null)}
        onSelect=${setSelectedTaskId}
      />
      ${paletteOpen ? html`<${CommandPalette} onClose=${() => setPaletteOpen(false)} />` : null}
    </div>
  `;
}

render(html`<${App} />`, document.getElementById("app"));
