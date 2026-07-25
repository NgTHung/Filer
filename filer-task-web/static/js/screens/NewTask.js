import { html, useEffect, useMemo, useRef, useState } from "../../vendor/preact-htm.js";
import { projectScoped } from "../api/client.js";
import { Header } from "../components/Header.js";
import { fieldError, nextNumber, preview } from "../lib/newTask.js";
import { domainNames, prefixesFor, tagCatalog, taskTypeNames } from "../lib/policy.js";

const PRIORITY_OPTIONS = ["High", "Medium", "Low"];

export function NewTaskScreen({ projectName, onCreated }) {
  const api = useMemo(() => projectScoped(projectName), [projectName]);
  const [policy, setPolicy] = useState(null);
  const [tasks, setTasks] = useState([]);
  const [milestones, setMilestones] = useState([]);
  const [draft, setDraft] = useState(emptyDraft());
  const [numberEdited, setNumberEdited] = useState(false);
  const [rejection, setRejection] = useState(null);
  const [submitting, setSubmitting] = useState(false);
  // Tracks whether the project active when a load/submit started is still the
  // active one, so a response that outlives a project switch cannot open the
  // wrong project's task drawer or repaint this screen with stale data.
  const guardRef = useRef({ cancelled: false });

  async function load(guard = guardRef.current) {
    try {
      const [loadedPolicy, loadedTasks, loadedMilestones] = await Promise.all([
        api.getPolicy(),
        api.getTasks(),
        api.getMilestones(),
      ]);
      if (guard.cancelled) {
        return;
      }
      setPolicy(loadedPolicy);
      setTasks(loadedTasks);
      setMilestones(loadedMilestones);
      setRejection(null);
    } catch (err) {
      if (!guard.cancelled) {
        setRejection(fieldError(err));
      }
    }
  }

  useEffect(() => {
    const guard = { cancelled: false };
    guardRef.current = guard;
    setDraft(emptyDraft());
    setNumberEdited(false);
    load(guard);
    return () => {
      guard.cancelled = true;
    };
  }, [projectName]);

  const domains = domainNames(policy);
  const prefixes = prefixesFor(policy, draft.domain);
  const types = taskTypeNames(policy);
  const catalog = tagCatalog(policy);

  // The first policy read decides the default domain, prefix and type, and any
  // domain change rescopes the prefix, because a prefix is only legal inside
  // the domain that declares it.
  useEffect(() => {
    if (!policy) {
      return;
    }
    const domain = domains.includes(draft.domain) ? draft.domain : (domains[0] ?? "");
    const domainPrefixes = prefixesFor(policy, domain);
    const prefix = domainPrefixes.includes(draft.prefix) ? draft.prefix : (domainPrefixes[0] ?? "");
    const type = types.includes(draft.type) ? draft.type : (types[0] ?? "");
    if (domain !== draft.domain || prefix !== draft.prefix || type !== draft.type) {
      setDraft((current) => ({ ...current, domain, prefix, type }));
    }
  }, [policy, draft.domain, draft.prefix, draft.type]);

  // The suggested number tracks the domain and prefix until the user overrides
  // it; after that their value stands, so a deliberate id is never overwritten.
  useEffect(() => {
    if (numberEdited || !draft.domain || !draft.prefix) {
      return;
    }
    setDraft((current) => ({ ...current, number: nextNumber(tasks, draft.domain, draft.prefix) }));
  }, [tasks, draft.domain, draft.prefix, numberEdited]);

  const identity = preview(draft.domain, draft.prefix, draft.number, draft.title);

  async function submit(event) {
    event.preventDefault();
    const guard = guardRef.current;
    setSubmitting(true);
    try {
      const view = await api.createTask({
        domain: draft.domain,
        prefix: draft.prefix,
        number: draft.number.trim(),
        title: draft.title.trim(),
        type: draft.type,
        priority: draft.priority,
        milestone: draft.milestone || null,
        tags: draft.tags,
      });
      if (guard.cancelled) {
        return;
      }
      setRejection(null);
      setDraft(emptyDraft());
      setNumberEdited(false);
      await load(guard);
      if (guard.cancelled) {
        return;
      }
      onCreated(view);
    } catch (err) {
      if (!guard.cancelled) {
        setRejection(fieldError(err));
      }
    } finally {
      setSubmitting(false);
    }
  }

  function update(field, value) {
    setDraft((current) => ({ ...current, [field]: value }));
  }

  function toggleTag(tag) {
    setDraft((current) => ({
      ...current,
      tags: current.tags.includes(tag)
        ? current.tags.filter((value) => value !== tag)
        : [...current.tags, tag],
    }));
  }

  const ready = Boolean(draft.domain && draft.prefix && draft.number.trim() && draft.title.trim() && draft.type);

  return html`
    <section class="screen">
      <${Header} title="New task" onRefresh=${load} />
      ${rejection && rejection.field === null
        ? html`<p class="screen-error">${rejection.message}</p>`
        : null}
      <form class="new-task-form" onSubmit=${submit}>
        <label>
          Domain
          <select value=${draft.domain} onChange=${(event) => update("domain", event.target.value)}>
            ${domains.map((value) => html`<option key=${value} value=${value}>${value}</option>`)}
          </select>
        </label>
        <label>
          Prefix
          <select value=${draft.prefix} onChange=${(event) => update("prefix", event.target.value)}>
            ${prefixes.map((value) => html`<option key=${value} value=${value}>${value}</option>`)}
          </select>
          <${FieldError} rejection=${rejection} field="prefix" />
        </label>
        <label>
          Number
          <input
            type="text"
            value=${draft.number}
            onInput=${(event) => {
              setNumberEdited(true);
              update("number", event.target.value);
            }}
          />
          <${FieldError} rejection=${rejection} field="number" />
        </label>
        <label>
          Title
          <input type="text" value=${draft.title} onInput=${(event) => update("title", event.target.value)} />
        </label>
        <label>
          Type
          <select value=${draft.type} onChange=${(event) => update("type", event.target.value)}>
            ${types.map((value) => html`<option key=${value} value=${value}>${value}</option>`)}
          </select>
          <${FieldError} rejection=${rejection} field="type" />
        </label>
        <label>
          Priority
          <select value=${draft.priority} onChange=${(event) => update("priority", event.target.value)}>
            ${PRIORITY_OPTIONS.map((value) => html`<option key=${value} value=${value}>${value}</option>`)}
          </select>
        </label>
        <label>
          Milestone
          <select value=${draft.milestone} onChange=${(event) => update("milestone", event.target.value)}>
            <option value="">None</option>
            ${milestoneValues(milestones).map((value) => html`<option key=${value} value=${value}>${value}</option>`)}
          </select>
        </label>
        <div class="new-task-tags">
          <span class="new-task-tags-label">Tags</span>
          ${catalog === null
            ? html`
                <input
                  type="text"
                  placeholder="comma separated"
                  value=${draft.tags.join(", ")}
                  onInput=${(event) => update("tags", parseTags(event.target.value))}
                />
              `
            : html`
                <div class="chip-row">
                  ${catalog.length === 0
                    ? html`<span class="muted-note">This project's tag catalog is empty.</span>`
                    : catalog.map(
                        (tag) => html`
                          <button
                            key=${tag}
                            type="button"
                            class="chip ${draft.tags.includes(tag) ? "chip-active" : ""}"
                            onClick=${() => toggleTag(tag)}
                          >
                            ${tag}
                          </button>
                        `,
                      )}
                </div>
              `}
          <${FieldError} rejection=${rejection} field="tags" />
        </div>
        <p class="new-task-preview">
          ${identity
            ? html`Creates <code>${identity.qualifiedId}</code> at <code>${identity.path}</code>`
            : "Pick a domain, prefix and number to preview the id and path."}
        </p>
        <div class="new-task-actions">
          <button type="submit" disabled=${!ready || submitting}>${submitting ? "Creating…" : "Create task"}</button>
        </div>
      </form>
    </section>
  `;
}

function FieldError({ rejection, field }) {
  if (!rejection || rejection.field !== field) {
    return null;
  }
  return html`
    <span class="field-error">
      ${rejection.message}
      ${rejection.allowed.length > 0 ? html`<span class="field-allowed">Allowed: ${rejection.allowed.join(", ")}</span>` : null}
    </span>
  `;
}

function emptyDraft() {
  return {
    domain: "",
    prefix: "",
    number: "",
    title: "",
    type: "",
    priority: "Medium",
    milestone: "",
    tags: [],
  };
}

function milestoneValues(aggregations) {
  return aggregations.map((entry) => entry.milestone.milestone).filter(Boolean);
}

function parseTags(value) {
  return value
    .split(",")
    .map((tag) => tag.trim())
    .filter(Boolean);
}
