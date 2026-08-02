import { html } from "../../vendor/preact-htm.js";

// Renders nothing unless the rejection named this exact field, so one rejection
// object can be handed to every input on a form.
export function FieldError({ rejection, field }) {
  if (!rejection || rejection.field !== field) {
    return null;
  }
  return html`
    <span class="field-error">
      ${rejection.message}
      ${rejection.allowed.length > 0
        ? html`<span class="field-allowed">Allowed: ${rejection.allowed.join(", ")}</span>`
        : null}
    </span>
  `;
}
