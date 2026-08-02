import { html } from "../../vendor/preact-htm.js";
import { milestoneOptions } from "../lib/milestones.js";
import { FieldError } from "./FieldError.js";

const RISK_OPTIONS = ["High", "Medium", "Low"];

// Status, priority and type are absent on purpose: status moves through the
// lifecycle actions, and the edit endpoint does not accept the other two.
export function DrawerEdit({ draft, milestones, rejection, saving, onChange, onSave, onCancel }) {
  function field(name) {
    return (event) => onChange(name, event.target.value);
  }

  return html`
    ${rejection && rejection.field === null
      ? html`<p class="screen-error" role="alert">${rejection.message}</p>`
      : null}
    <form
      class="drawer-edit-form"
      onSubmit=${(event) => {
        event.preventDefault();
        onSave();
      }}
    >
      <label>
        Title
        <input type="text" value=${draft.title} onInput=${field("title")} />
        <${FieldError} rejection=${rejection} field="title" />
      </label>
      <label>
        Summary
        <textarea rows="5" value=${draft.summary} onInput=${field("summary")}></textarea>
        <${FieldError} rejection=${rejection} field="summary" />
      </label>
      <label>
        Risk
        <select value=${draft.risk} onChange=${field("risk")}>
          <option value="">None</option>
          ${RISK_OPTIONS.map((value) => html`<option key=${value} value=${value}>${value}</option>`)}
        </select>
        <${FieldError} rejection=${rejection} field="risk" />
      </label>
      <label>
        Impact
        <textarea rows="3" value=${draft.impact} onInput=${field("impact")}></textarea>
        <${FieldError} rejection=${rejection} field="impact" />
      </label>
      <label>
        Tags
        <input
          type="text"
          placeholder="comma separated"
          value=${draft.tagsText}
          onInput=${field("tagsText")}
        />
        <${FieldError} rejection=${rejection} field="tags" />
      </label>
      <label>
        Milestone
        <select value=${draft.milestone} onChange=${field("milestone")}>
          <option value="">None</option>
          ${milestoneOptions(milestones, draft.milestone).map(
            (value) => html`<option key=${value} value=${value}>${value}</option>`,
          )}
        </select>
        <${FieldError} rejection=${rejection} field="milestone" />
      </label>
      <label>
        Parent
        <input
          type="text"
          placeholder="WEB-001 or other-domain:CORE-001"
          value=${draft.parent}
          onInput=${field("parent")}
        />
        <${FieldError} rejection=${rejection} field="parent" />
      </label>
      <label>
        Depends on
        <input
          type="text"
          placeholder="comma separated"
          value=${draft.dependsText}
          onInput=${field("dependsText")}
        />
        <${FieldError} rejection=${rejection} field="depends_on" />
      </label>
      <div class="drawer-edit-actions">
        <button type="submit" disabled=${saving}>${saving ? "Saving…" : "Save changes"}</button>
        <button type="button" onClick=${onCancel} disabled=${saving}>Cancel</button>
      </div>
    </form>
  `;
}
