import { html, useState } from "../../vendor/preact-htm.js";
import { tagCatalog } from "../lib/policy.js";
import { addTagOperation, removeTagOperation, tagFlipWarning } from "../lib/policyOps.js";
import { sectionRejection } from "../lib/policyRejection.js";
import { RejectionNotice } from "./RejectionNotice.js";

export function PolicyTags({ policy, rejection, onSubmit, busy }) {
  const [tag, setTag] = useState("");

  const added = addTagOperation(tag);
  const catalog = tagCatalog(policy);
  const warning = tagFlipWarning(policy);

  function submit(event) {
    event.preventDefault();
    onSubmit(added).then((accepted) => {
      if (accepted) {
        setTag("");
      }
    });
  }

  return html`
    <section class="policy-section">
      <h3 class="settings-heading">Tags</h3>
      <${RejectionNotice} rejection=${sectionRejection(rejection, "tags")} />
      ${warning ? html`<p class="muted-note">${warning}</p>` : null}
      ${catalog === null
        ? null
        : html`
            <div class="chip-row">
              ${catalog.length === 0
                ? html`<span class="muted-note">This project's tag catalog is empty.</span>`
                : catalog.map(
                    (value) => html`
                      <span class="policy-chip" key=${value}>
                        ${value}
                        <button
                          type="button"
                          class="chip-remove"
                          title=${`Remove tag ${value}`}
                          disabled=${busy}
                          onClick=${() => onSubmit(removeTagOperation(value))}
                        >
                          ×
                        </button>
                      </span>
                    `,
                  )}
            </div>
          `}
      <form class="policy-add" onSubmit=${submit}>
        <input
          type="text"
          placeholder="new tag"
          value=${tag}
          onInput=${(event) => setTag(event.currentTarget.value)}
        />
        <button type="submit" disabled=${!added || busy}>Add tag</button>
      </form>
    </section>
  `;
}
