import { html } from "../../vendor/preact-htm.js";

// The overlay, the outside click, and the Escape key, shared by every modal so
// they behave the same wherever they are mounted. Both cancel paths stop the
// event: a dialog opened from the switcher sits inside the switcher's own
// overlay, which would otherwise read the same click or key as "close the
// switcher too".
export function Dialog({ title, onCancel, children }) {
  function cancel(event) {
    event.stopPropagation();
    onCancel();
  }

  function keyDown(event) {
    if (event.key === "Escape") {
      event.preventDefault();
      cancel(event);
    }
  }

  return html`
    <div class="dialog-overlay" onClick=${cancel}>
      <div
        class="dialog-panel"
        onClick=${(event) => event.stopPropagation()}
        onKeyDown=${keyDown}
      >
        <h3 class="dialog-title">${title}</h3>
        ${children}
      </div>
    </div>
  `;
}
