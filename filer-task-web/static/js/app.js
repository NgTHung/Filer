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
import { SettingsScreen } from "./screens/Settings.js";
import { TaskDrawer } from "./components/TaskDrawer.js";
import { isPaletteShortcut } from "./lib/palette.js";
import { loadIdentity, useIdentityStore } from "./store/identity.js";

function Screen({ screen, project, onSelectTask }) {
  // First, because it is the one screen that renders without a project.
  if (screen === "settings") {
    return html`<${SettingsScreen} projectName=${project?.name} />`;
  }
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

  // Sticky, so that opening the first project from the forced Settings screen
  // leaves the user on Settings instead of bouncing them to Ready. Gated on the
  // load finishing, because every page load starts with no project and would
  // otherwise strand the user on Settings.
  useEffect(() => {
    if (!store.loading && !project) {
      setScreen("settings");
    }
  }, [store.loading, Boolean(project)]);

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

  // Settings is the only screen that works without a project, and it is where
  // the first one gets registered, so an empty registry lands there instead of
  // on a dead end. A broken project needs the same escape.
  const active = project ? screen : "settings";

  return html`
    <div class="app-shell">
      <${Sidebar}
        screen=${active}
        onSelectScreen=${setScreen}
        onSwitchProject=${() => setPaletteOpen(true)}
      />
      <main class="app-main">
        ${project && project.broken && active !== "settings"
          ? html`<${BrokenScreen}
              project=${project}
              onSwitchProject=${() => setPaletteOpen(true)}
              onOpenSettings=${() => setScreen("settings")}
            />`
          : html`<${Screen} screen=${active} project=${project} onSelectTask=${setSelectedTaskId} />`}
      </main>
      <${TaskDrawer}
        projectName=${project?.name}
        taskId=${selectedTaskId}
        onClose=${() => setSelectedTaskId(null)}
        onSelect=${setSelectedTaskId}
      />
      ${paletteOpen ? html`<${CommandPalette} onClose=${() => setPaletteOpen(false)} />` : null}
    </div>
  `;
}

render(html`<${App} />`, document.getElementById("app"));
