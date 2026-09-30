import { html } from "../../vendor/preact-htm.js";

// Each row carries its own content hash, so a toggle targets the exact source
// line the drawer rendered rather than whatever now sits at that index.
export function DrawerCriteria({ heading, criteria, refusal, pendingIndex, onToggle }) {
  const highlighted = new Set(refusal ? refusal.indexes : []);

  return html`
    <section class="drawer-section">
      <h4>${heading}</h4>
      ${criteria.length === 0
        ? html`<p class="muted-note">None listed.</p>`
        : html`
            <ul class="criteria-list">
              ${criteria.map(
                (item, index) => html`
                  <li
                    key=${`${index}-${item.content_hash}`}
                    class=${criterionClass(item.checked, highlighted.has(index))}
                  >
                    <label class="criterion-label">
                      <input
                        type="checkbox"
                        checked=${item.checked}
                        disabled=${pendingIndex !== null}
                        onChange=${(event) => onToggle(index, event.currentTarget.checked)}
                      />
                      <span>${item.text}</span>
                    </label>
                  </li>
                `,
              )}
            </ul>
          `}
    </section>
  `;
}

function criterionClass(checked, highlighted) {
  const classes = [];
  if (checked) {
    classes.push("criterion-checked");
  }
  if (highlighted) {
    classes.push("criterion-unmet");
  }
  return classes.join(" ");
}
