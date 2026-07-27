import { html, useState } from "../../vendor/preact-htm.js";
import { RejectionNotice } from "./RejectionNotice.js";

// The path is whatever the user pastes; the server walks up from it to find the
// project root, so this form never tries to interpret it.
export function ProjectOpenForm({ onOpen, busy, rejection }) {
  const [path, setPath] = useState("");
  const [init, setInit] = useState(false);

  async function submit(event) {
    event.preventDefault();
    if (await onOpen(path.trim(), init)) {
      setPath("");
      setInit(false);
    }
  }

  return html`
    <form class="settings-form" onSubmit=${submit}>
      <label>
        Project path
        <input
          type="text"
          placeholder="an absolute path to a project directory"
          value=${path}
          onInput=${(event) => setPath(event.currentTarget.value)}
        />
      </label>
      <label class="settings-checkbox">
        <input
          type="checkbox"
          checked=${init}
          onChange=${(event) => setInit(event.currentTarget.checked)}
        />
        <span>Create new here</span>
      </label>
      <div class="settings-actions">
        <button type="submit" disabled=${!path.trim() || busy}>${busy ? "Opening…" : "Open"}</button>
      </div>
      <${RejectionNotice} rejection=${rejection} />
    </form>
  `;
}
