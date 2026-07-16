import { html, render, useState } from "../vendor/preact-htm.js";
import { activeProject, loadProjects, useProjectStore } from "./store/project.js";
import { Sidebar } from "./components/Sidebar.js";
import { IdentityPrompt } from "./components/IdentityPrompt.js";
import { ProjectSwitcher } from "./components/ProjectSwitcher.js";
import { ReadyScreen } from "./screens/Ready.js";
import { BrokenScreen } from "./screens/Broken.js";
import { loadIdentity, useIdentityStore } from "./store/identity.js";

function Screen({ screen, project }) {
  if (screen === "ready") {
    return html`<${ReadyScreen} projectName=${project.name} />`;
  }
  // Tasks, Milestones, and New task ship in WEB-006/007.
  return html`<p class="empty-state">This screen is not built yet.</p>`;
}

function App() {
  const identityStore = useIdentityStore();
  const store = useProjectStore();
  const [screen, setScreen] = useState("ready");
  const [switcherOpen, setSwitcherOpen] = useState(false);
  const project = activeProject();

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
          : html`<${Screen} screen=${screen} project=${project} />`}
      </main>
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
