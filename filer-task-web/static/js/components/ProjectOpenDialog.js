import { html, useEffect, useRef, useState } from "../../vendor/preact-htm.js";
import { Dialog } from "./Dialog.js";
import { RejectionNotice } from "./RejectionNotice.js";

// The path is whatever the user pastes; the server walks up from it to find the
// project root, so this dialog never tries to interpret it. Creating lives in
// its own dialog, so nothing here can register a project that is not there yet.
export function ProjectOpenDialog({ onOpen, onCancel }) {
  const [path, setPath] = useState("");
  const [busy, setBusy] = useState(false);
  const [rejection, setRejection] = useState(null);
  const pathRef = useRef(null);

  useEffect(() => {
    if (pathRef.current) {
      pathRef.current.focus();
    }
  }, []);

  async function submit(event) {
    event.preventDefault();
    setBusy(true);
    setRejection(null);
    const result = await onOpen(path.trim());
    setBusy(false);
    if (!result.ok) {
      setRejection(result.rejection);
    }
  }

  return html`
    <${Dialog} title="Open a project" onCancel=${onCancel}>
      <form class="dialog-form" onSubmit=${submit}>
        <label>
          Path
          <input
            ref=${pathRef}
            type="text"
            placeholder="an absolute path inside an existing project"
            value=${path}
            onInput=${(event) => setPath(event.currentTarget.value)}
          />
        </label>
        <${RejectionNotice} rejection=${rejection} />
        <div class="dialog-actions">
          <button type="button" onClick=${onCancel} disabled=${busy}>Cancel</button>
          <button type="submit" disabled=${busy || !path.trim()}>
            ${busy ? "Opening…" : "Open"}
          </button>
        </div>
      </form>
    <//>
  `;
}
