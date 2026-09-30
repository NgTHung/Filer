import { html, useEffect, useRef, useState } from "../../vendor/preact-htm.js";
import { projectRootPreview } from "../lib/registration.js";
import { Dialog } from "./Dialog.js";
import { FieldError } from "./FieldError.js";

// Asks for the two things creating a project needs: what to call it, and which
// directory to make it in. The name is a directory name, so the server refuses
// anything that reaches out of the chosen location and names the offending
// field, which is why the refusal renders beside an input rather than above the
// form.
export function ProjectCreateDialog({ proposedName, onCreate, onCancel }) {
  const [name, setName] = useState(proposedName ?? "");
  const [location, setLocation] = useState("");
  const [busy, setBusy] = useState(false);
  const [rejection, setRejection] = useState(null);
  const locationRef = useRef(null);

  // A name proposed by the switcher query leaves the location as the field the
  // user still has to fill in.
  useEffect(() => {
    const target = proposedName ? locationRef.current : null;
    if (target) {
      target.focus();
    }
  }, []);

  async function submit(event) {
    event.preventDefault();
    setBusy(true);
    setRejection(null);
    const result = await onCreate(location.trim(), name.trim());
    setBusy(false);
    if (!result.ok) {
      setRejection(result.rejection);
    }
  }

  return html`
    <${Dialog} title="Create a project" onCancel=${onCancel}>
      <form class="dialog-form" onSubmit=${submit}>
        <label>
          Name
          <input
            type="text"
            value=${name}
            onInput=${(event) => setName(event.currentTarget.value)}
          />
        </label>
        <${FieldError} rejection=${rejection} field="name" />
        <label>
          Location
          <input
            ref=${locationRef}
            type="text"
            placeholder="an absolute path to the directory to create it in"
            value=${location}
            onInput=${(event) => setLocation(event.currentTarget.value)}
          />
        </label>
        <${FieldError} rejection=${rejection} field="path" />
        ${rejection && rejection.field === null
          ? html`<p class="screen-error">${rejection.message}</p>`
          : null}
        <p class="muted-note">The project is created at ${projectRootPreview(location, name)}</p>
        <div class="dialog-actions">
          <button type="button" onClick=${onCancel} disabled=${busy}>Cancel</button>
          <button type="submit" disabled=${busy || !name.trim() || !location.trim()}>
            ${busy ? "Creating…" : "Create"}
          </button>
        </div>
      </form>
    <//>
  `;
}
