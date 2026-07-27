import { html, useState } from "../../vendor/preact-htm.js";
import { taskTypeNames } from "../lib/policy.js";
import { addTaskTypeOperation, removeTaskTypeOperation } from "../lib/policyOps.js";
import { sectionRejection } from "../lib/policyRejection.js";
import { RejectionNotice } from "./RejectionNotice.js";

const CRITERIA_OPTIONS = ["acceptance", "exit"];

export function PolicyTaskTypes({ policy, rejection, onSubmit, busy }) {
  const [name, setName] = useState("");
  const [criteria, setCriteria] = useState("acceptance");
  const [milestone, setMilestone] = useState(false);

  const added = addTaskTypeOperation(name, criteria, milestone);

  function submit(event) {
    event.preventDefault();
    onSubmit(added).then((accepted) => {
      if (accepted) {
        setName("");
        setCriteria("acceptance");
        setMilestone(false);
      }
    });
  }

  return html`
    <section class="policy-section">
      <h3 class="settings-heading">Task types</h3>
      <${RejectionNotice} rejection=${sectionRejection(rejection, "task_types")} />
      ${taskTypeNames(policy).map((type) => {
        const detail = policy.task_types[type];
        return html`
          <div class="policy-row-head" key=${type}>
            <span class="policy-name">${type}</span>
            <span class="muted-note">
              ${detail.criteria}${detail.role ? ` · ${detail.role}` : ""}
            </span>
            <button type="button" disabled=${busy} onClick=${() => onSubmit(removeTaskTypeOperation(type))}>
              Remove
            </button>
          </div>
        `;
      })}
      <form class="policy-add" onSubmit=${submit}>
        <input
          type="text"
          placeholder="new task type"
          value=${name}
          onInput=${(event) => setName(event.currentTarget.value)}
        />
        <select value=${criteria} onChange=${(event) => setCriteria(event.currentTarget.value)}>
          ${CRITERIA_OPTIONS.map((value) => html`<option key=${value} value=${value}>${value}</option>`)}
        </select>
        <label class="settings-checkbox">
          <input
            type="checkbox"
            checked=${milestone}
            onChange=${(event) => setMilestone(event.currentTarget.checked)}
          />
          <span>Milestone role</span>
        </label>
        <button type="submit" disabled=${!added || busy}>Add task type</button>
      </form>
    </section>
  `;
}
