import { html, useState } from "../../vendor/preact-htm.js";
import { activeFilterEntries } from "../lib/filters.js";

const STATUS_OPTIONS = ["To Do", "In Progress", "Blocked", "Done", "Deferred", "Obsolete"];
const PRIORITY_OPTIONS = ["High", "Medium", "Low"];

const FIELD_LABELS = {
  status: "Status",
  priority: "Priority",
  domain: "Domain",
  parent: "Parent",
  milestone: "Milestone",
  tag: "Tag",
  blocked: "Blocked only",
};

function emptyDraft(applied) {
  return {
    status: applied.status ?? "",
    priority: applied.priority ?? "",
    domain: applied.domain ?? "",
    parent: applied.parent ?? "",
    milestone: applied.milestone ?? "",
    tag: applied.tag ?? "",
    blocked: applied.blocked ?? false,
  };
}

// Popover form covering every /tasks query param, plus a chip row summarizing
// applied filters and any rejected value the backend sent back with an error.
export function FilterMenu({ policy, applied, rejected, onApply, onClear }) {
  const [open, setOpen] = useState(false);
  const [draft, setDraft] = useState(() => emptyDraft(applied));

  function openMenu() {
    setDraft(emptyDraft(applied));
    setOpen(true);
  }

  function submit(event) {
    event.preventDefault();
    onApply({
      status: draft.status || undefined,
      priority: draft.priority || undefined,
      domain: draft.domain.trim() || undefined,
      parent: draft.parent.trim() || undefined,
      milestone: draft.milestone.trim() || undefined,
      tag: draft.tag.trim() || undefined,
      blocked: draft.blocked || undefined,
    });
    setOpen(false);
  }

  const strictTags = policy && policy.tags && policy.tags.policy === "strict";
  const tagOptions = strictTags ? policy.tags.allowed ?? [] : null;

  const appliedEntries = activeFilterEntries(applied);
  const rejections = rejected ?? [];

  return html`
    <div class="filter-menu">
      <div class="chip-row">
        <button class="filter-menu-trigger" onClick=${openMenu}>+ Filter</button>
        ${appliedEntries.map(
          ([field, value]) => html`
            <button
              key=${field}
              class="chip chip-active"
              onClick=${() => onApply({ ...applied, [field]: undefined })}
              title="Remove filter"
            >
              ${FIELD_LABELS[field]}: ${value === true ? "yes" : value} ×
            </button>
          `,
        )}
        ${rejections.map(
          (rejection) => html`
            <span key=${rejection.field} class="error-pill">
              ${FIELD_LABELS[rejection.field] ?? rejection.field}: ${rejection.message}
              ${(rejection.candidates ?? []).map(
                (candidate) => html`
                  <button
                    key=${candidate}
                    class="candidate-chip"
                    onClick=${() => onApply({ ...applied, [rejection.field]: candidate })}
                  >
                    ${candidate}
                  </button>
                `,
              )}
            </span>
          `,
        )}
        ${appliedEntries.length > 0 || rejections.length > 0
          ? html`<button class="filter-clear" onClick=${onClear}>Clear filters</button>`
          : null}
      </div>
      ${open
        ? html`
            <div class="filter-menu-overlay" onClick=${() => setOpen(false)}>
              <form
                class="filter-menu-panel"
                onClick=${(event) => event.stopPropagation()}
                onSubmit=${submit}
              >
                <h3>Filter tasks</h3>
                <label>
                  Status
                  <select
                    value=${draft.status}
                    onChange=${(event) => setDraft({ ...draft, status: event.target.value })}
                  >
                    <option value="">Any</option>
                    ${STATUS_OPTIONS.map((value) => html`<option value=${value}>${value}</option>`)}
                  </select>
                </label>
                <label>
                  Priority
                  <select
                    value=${draft.priority}
                    onChange=${(event) => setDraft({ ...draft, priority: event.target.value })}
                  >
                    <option value="">Any</option>
                    ${PRIORITY_OPTIONS.map((value) => html`<option value=${value}>${value}</option>`)}
                  </select>
                </label>
                <label>
                  Domain
                  <input
                    type="text"
                    value=${draft.domain}
                    onInput=${(event) => setDraft({ ...draft, domain: event.target.value })}
                  />
                </label>
                <label>
                  Parent
                  <input
                    type="text"
                    placeholder="domain:LOCAL-ID"
                    value=${draft.parent}
                    onInput=${(event) => setDraft({ ...draft, parent: event.target.value })}
                  />
                </label>
                <label>
                  Milestone
                  <input
                    type="text"
                    value=${draft.milestone}
                    onInput=${(event) => setDraft({ ...draft, milestone: event.target.value })}
                  />
                </label>
                <label>
                  Tag
                  ${tagOptions
                    ? html`
                        <select
                          value=${draft.tag}
                          onChange=${(event) => setDraft({ ...draft, tag: event.target.value })}
                        >
                          <option value="">Any</option>
                          ${tagOptions.map((value) => html`<option value=${value}>${value}</option>`)}
                        </select>
                      `
                    : html`
                        <input
                          type="text"
                          value=${draft.tag}
                          onInput=${(event) => setDraft({ ...draft, tag: event.target.value })}
                        />
                      `}
                </label>
                <label class="filter-checkbox">
                  <input
                    type="checkbox"
                    checked=${draft.blocked}
                    onChange=${(event) => setDraft({ ...draft, blocked: event.target.checked })}
                  />
                  Blocked only
                </label>
                <div class="filter-menu-actions">
                  <button type="button" onClick=${() => setOpen(false)}>Cancel</button>
                  <button type="submit">Apply</button>
                </div>
              </form>
            </div>
          `
        : null}
    </div>
  `;
}
