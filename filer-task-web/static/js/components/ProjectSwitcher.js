import { html } from "../../vendor/preact-htm.js";
import { setActiveProject, useProjectStore } from "../store/project.js";

// Minimal project-switcher overlay. WEB-008 replaces this with the full
// command-K palette; this stub only needs to let WEB-005 change the active project.
export function ProjectSwitcher({ onClose }) {
  const store = useProjectStore();

  function select(name) {
    setActiveProject(name);
    onClose();
  }

  return html`
    <div class="switcher-overlay" onClick=${onClose}>
      <div class="switcher-panel" onClick=${(event) => event.stopPropagation()}>
        <h3>Switch project</h3>
        <ul class="switcher-list">
          ${store.projects.map(
            (project) => html`
              <li key=${project.name}>
                <button class="switcher-item" onClick=${() => select(project.name)}>
                  <span>${project.name}</span>
                  ${project.broken ? html`<span class="switcher-broken">broken</span>` : null}
                </button>
              </li>
            `,
          )}
        </ul>
      </div>
    </div>
  `;
}
