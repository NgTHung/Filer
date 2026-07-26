import { html, useEffect, useMemo, useRef, useState } from "../../vendor/preact-htm.js";
import { projectScoped } from "../api/client.js";
import { doneRefusal, isConflict } from "../lib/taskDetail.js";
import { DrawerActions } from "./DrawerActions.js";
import { DrawerCriteria } from "./DrawerCriteria.js";
import { DrawerRelations } from "./DrawerRelations.js";

// The drawer owns its own reads: relationship chips reselect by setting one id,
// and every write path refreshes through the same request, so callers never
// hold a view that the drawer has already moved past.
export function TaskDrawer({ projectName, taskId, onClose, onSelect }) {
  const api = useMemo(() => projectScoped(projectName), [projectName]);
  const [context, setContext] = useState(null);
  const [error, setError] = useState(null);
  const [notice, setNotice] = useState(null);
  const [refusal, setRefusal] = useState(null);
  const [pendingAction, setPendingAction] = useState(null);
  const [pendingCriterion, setPendingCriterion] = useState(null);
  // A response that outlives a reselect or a project switch would repaint the
  // drawer with the task the user already navigated away from.
  const guardRef = useRef({ cancelled: false });

  async function load(guard) {
    try {
      const loaded = await api.getTaskContext(taskId);
      if (!guard.cancelled) {
        setContext(loaded);
        setError(null);
      }
    } catch (err) {
      if (!guard.cancelled) {
        setContext(null);
        setError(err);
      }
    }
  }

  useEffect(() => {
    if (!taskId) {
      return undefined;
    }
    const guard = { cancelled: false };
    guardRef.current = guard;
    setContext(null);
    setError(null);
    setNotice(null);
    setRefusal(null);
    load(guard);
    return () => {
      guard.cancelled = true;
    };
  }, [projectName, taskId]);

  if (!taskId) {
    return null;
  }

  const criteria = context ? context.detail.criteria : [];

  // Refusing Done here keeps the endpoint uncalled: the server rejects it too,
  // but only its heading, never which items are still open.
  async function runAction(action, reason) {
    if (action === "done") {
      const unmet = doneRefusal(criteria);
      if (unmet) {
        setRefusal(unmet);
        setNotice(null);
        return;
      }
    }
    const guard = guardRef.current;
    setRefusal(null);
    setNotice(null);
    setPendingAction(action);
    try {
      const view = await api.transition(taskId, action, reason);
      if (guard.cancelled) {
        return;
      }
      applyView(view);
      await load(guard);
    } catch (err) {
      if (!guard.cancelled) {
        setNotice(err.message);
      }
    } finally {
      setPendingAction(null);
    }
  }

  async function toggleCriterion(index, checked) {
    const guard = guardRef.current;
    setRefusal(null);
    setNotice(null);
    setPendingCriterion(index);
    try {
      const view = await api.setCriterion(taskId, index, checked, criteria[index].content_hash);
      if (!guard.cancelled) {
        applyView(view);
      }
    } catch (err) {
      if (guard.cancelled) {
        return;
      }
      setNotice(
        isConflict(err)
          ? `This criterion changed since it was loaded, so the toggle was not applied: ${err.message}`
          : err.message,
      );
      await load(guard);
    } finally {
      setPendingCriterion(null);
    }
  }

  // Writes answer with a ShowView, which refreshes the body immediately; the
  // graph and readiness around it come from the reload that follows.
  function applyView(view) {
    setContext((current) =>
      current ? { ...current, detail: view.detail, warnings: view.warnings } : current,
    );
  }

  return html`
    <div class="drawer-overlay" onClick=${onClose}>
      <aside class="drawer" onClick=${(event) => event.stopPropagation()}>
        ${error
          ? html`
              <header class="drawer-header">
                <span class="drawer-id">${taskId}</span>
                <button class="drawer-close" onClick=${onClose} title="Close">×</button>
              </header>
              <p class="screen-error">Could not load ${taskId}: ${error.message}</p>
            `
          : !context
            ? html`<p class="muted-note">Loading ${taskId}…</p>`
            : html`<${DrawerBody}
                context=${context}
                notice=${notice}
                refusal=${refusal}
                pendingAction=${pendingAction}
                pendingCriterion=${pendingCriterion}
                onClose=${onClose}
                onSelect=${onSelect}
                onRun=${runAction}
                onToggle=${toggleCriterion}
              />`}
      </aside>
    </div>
  `;
}

function DrawerBody({
  context,
  notice,
  refusal,
  pendingAction,
  pendingCriterion,
  onClose,
  onSelect,
  onRun,
  onToggle,
}) {
  const { task, sections, criteria, criteria_heading } = context.detail;

  return html`
    <header class="drawer-header">
      <span class="drawer-id">${task.qualified_id}</span>
      <span class="drawer-badge">${task.status}</span>
      <span class="drawer-badge">${task.priority}</span>
      <button class="drawer-close" onClick=${onClose} title="Close">×</button>
    </header>
    <h3 class="drawer-title">${task.title}</h3>
    <${DrawerActions} pendingAction=${pendingAction} onRun=${onRun} />
    ${refusal
      ? html`
          <p class="drawer-refusal" role="alert">
            ${refusal.count} of ${criteria.length} criteria are still unchecked, so Done was not
            requested. Check every highlighted item first.
          </p>
        `
      : null}
    ${notice ? html`<p class="drawer-notice" role="alert">${notice}</p>` : null}
    <dl class="drawer-meta">
      <dt>Type</dt>
      <dd>${task.type}</dd>
      <dt>Domain</dt>
      <dd>${task.domain}</dd>
      ${task.milestone ? html`<dt>Milestone</dt><dd>${task.milestone}</dd>` : null}
      ${(task.tags ?? []).length > 0 ? html`<dt>Tags</dt><dd>${task.tags.join(", ")}</dd>` : null}
      <dt>Path</dt>
      <dd class="drawer-path">${task.path}</dd>
    </dl>
    <${DrawerRelations} context=${context} onSelect=${onSelect} />
    ${sections.map(
      (section, index) => html`
        <section key=${`${index}-${section.heading}`} class="drawer-section">
          <h4>${section.heading}</h4>
          <p>${section.content}</p>
        </section>
      `,
    )}
    <${DrawerCriteria}
      heading=${criteria_heading}
      criteria=${criteria}
      refusal=${refusal}
      pendingIndex=${pendingCriterion}
      onToggle=${onToggle}
    />
    ${context.warnings.length > 0
      ? html`
          <section class="drawer-section">
            <h4>Warnings</h4>
            <ul class="issue-list">
              ${context.warnings.map((warning, index) => html`<li key=${index}>${warning.message}</li>`)}
            </ul>
          </section>
        `
      : null}
  `;
}
