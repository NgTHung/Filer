import { html, useEffect, useRef, useState } from "../../vendor/preact-htm.js";
import { filterProjects, moveSelection } from "../lib/palette.js";
import { setActiveProject, useProjectStore } from "../store/project.js";

// Cmd/Ctrl-K project switcher. Switching the active project rescopes every API
// path through projectScoped, so this overlay is the only place that needs to
// know a switch happened.
export function CommandPalette({ onClose }) {
  const store = useProjectStore();
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);
  const inputRef = useRef(null);

  const matches = filterProjects(store.projects, query);

  useEffect(() => {
    if (inputRef.current) {
      inputRef.current.focus();
    }
  }, []);

  function keyDown(event) {
    if (event.key === "Escape") {
      event.preventDefault();
      onClose();
      return;
    }
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      setSelected((current) =>
        moveSelection(current, event.key === "ArrowDown" ? 1 : -1, matches.length),
      );
      return;
    }
    if (event.key === "Enter") {
      event.preventDefault();
      select(matches[selected]);
    }
  }

  function select(project) {
    if (!project) {
      return;
    }
    setActiveProject(project.name);
    onClose();
  }

  return html`
    <div class="switcher-overlay" onClick=${onClose}>
      <div class="switcher-panel" onClick=${(event) => event.stopPropagation()} onKeyDown=${keyDown}>
        <input
          ref=${inputRef}
          class="switcher-query"
          type="text"
          placeholder="Switch project…"
          value=${query}
          onInput=${(event) => {
            setQuery(event.currentTarget.value);
            setSelected(0);
          }}
        />
        ${matches.length === 0
          ? html`<p class="muted-note">No project matches ${query}.</p>`
          : html`
              <ul class="switcher-list">
                ${matches.map(
                  (project, index) => html`
                    <li key=${project.name}>
                      <button
                        type="button"
                        class="switcher-item ${index === selected ? "switcher-item-active" : ""}"
                        onMouseEnter=${() => setSelected(index)}
                        onClick=${() => select(project)}
                      >
                        <span>${project.name}</span>
                        ${project.broken ? html`<span class="switcher-broken">broken</span>` : null}
                      </button>
                    </li>
                  `,
                )}
              </ul>
            `}
      </div>
    </div>
  `;
}
