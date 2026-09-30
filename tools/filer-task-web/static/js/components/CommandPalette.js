import { html, useEffect, useRef, useState } from "../../vendor/preact-htm.js";
import { moveSelection, paletteRows } from "../lib/palette.js";
import { openProject } from "../lib/projectOpen.js";
import { fieldError } from "../lib/rejection.js";
import { setActiveProject, useProjectStore } from "../store/project.js";
import { ProjectCreateDialog } from "./ProjectCreateDialog.js";

// Cmd/Ctrl-K project switcher. Switching the active project rescopes every API
// path through projectScoped, so this overlay is the only place that needs to
// know a switch happened. A query that names nothing registered becomes the
// proposed name for a new project, so an unregistered project can be created
// without a detour through Settings.
export function CommandPalette({ onClose }) {
  const store = useProjectStore();
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);
  const [creating, setCreating] = useState(null);
  const inputRef = useRef(null);

  const rows = paletteRows(store.projects, query);

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
        moveSelection(current, event.key === "ArrowDown" ? 1 : -1, rows.length),
      );
      return;
    }
    if (event.key === "Enter") {
      event.preventDefault();
      select(rows[selected]);
    }
  }

  function select(row) {
    if (!row || creating !== null) {
      return;
    }
    if (row.kind === "create") {
      setCreating(row.name);
      return;
    }
    setActiveProject(row.project.name);
    onClose();
  }

  // A refusal stays in the dialog, beside the field the server named, because
  // the fix is a correction to what was typed there.
  async function create(location, name) {
    const result = await openProject(location, true, name);
    if (result.ok) {
      onClose();
      return result;
    }
    return { ok: false, rejection: fieldError(result.error) };
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
        ${rows.length === 0
          ? html`<p class="muted-note">Type a project name to switch to it, or to create it.</p>`
          : html`
              <ul class="switcher-list">
                ${rows.map(
                  (row, index) => html`
                    <li key=${row.kind === "create" ? "create" : row.project.name}>
                      <button
                        type="button"
                        class="switcher-item ${index === selected ? "switcher-item-active" : ""}"
                        onMouseEnter=${() => setSelected(index)}
                        onClick=${() => select(row)}
                      >
                        <${RowLabel} row=${row} />
                      </button>
                    </li>
                  `,
                )}
              </ul>
            `}
      </div>
      ${creating === null
        ? null
        : html`<${ProjectCreateDialog}
            proposedName=${creating}
            onCreate=${create}
            onCancel=${() => setCreating(null)}
          />`}
    </div>
  `;
}

function RowLabel({ row }) {
  if (row.kind === "create") {
    return html`
      <span>Create a project named <code>${row.name}</code></span>
      <span class="switcher-create">new</span>
    `;
  }
  return html`
    <span>${row.project.name}</span>
    ${row.project.broken ? html`<span class="switcher-broken">broken</span>` : null}
  `;
}
