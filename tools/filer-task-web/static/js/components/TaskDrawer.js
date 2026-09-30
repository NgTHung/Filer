import { html, useEffect, useMemo, useRef, useState } from "../../vendor/preact-htm.js";
import { projectScoped } from "../api/client.js";
import { fieldError } from "../lib/rejection.js";
import { doneRefusal, isConflict } from "../lib/taskDetail.js";
import { editDraft, editPatch } from "../lib/taskEdit.js";
import { DrawerActions } from "./DrawerActions.js";
import { DrawerCriteria } from "./DrawerCriteria.js";
import { DrawerEdit } from "./DrawerEdit.js";
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
  // `baseline` is the draft as it was loaded; the patch is the difference
  // between the two, so an untouched field is never sent and never overwritten.
  const [draft, setDraft] = useState(null);
  const [baseline, setBaseline] = useState(null);
  const [editRejection, setEditRejection] = useState(null);
  const [saving, setSaving] = useState(false);
  const [milestones, setMilestones] = useState([]);
  // A response that outlives a reselect or a project switch would repaint the
  // drawer with the task the user already navigated away from.
  const guardRef = useRef({ cancelled: false });
  // A project with no milestones is a real answer, so emptiness cannot double as
  // "not fetched yet" without refetching on every Edit.
  const milestonesLoadedRef = useRef(false);

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
    closeEdit();
    load(guard);
    return () => {
      guard.cancelled = true;
    };
  }, [projectName, taskId]);

  // Milestone options belong to the project, not the task, so they survive a
  // reselect and are only dropped when the project underneath them changes.
  useEffect(() => {
    milestonesLoadedRef.current = false;
    setMilestones([]);
  }, [projectName]);

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

  function closeEdit() {
    setDraft(null);
    setBaseline(null);
    setEditRejection(null);
  }

  async function beginEdit() {
    const loaded = editDraft(context.detail);
    setBaseline(loaded);
    setDraft(loaded);
    setEditRejection(null);
    setNotice(null);
    setRefusal(null);
    if (milestonesLoadedRef.current) {
      return;
    }
    const guard = guardRef.current;
    try {
      const options = await api.getMilestones();
      if (!guard.cancelled) {
        milestonesLoadedRef.current = true;
        setMilestones(options);
      }
    } catch {
      // The milestone list is an affordance, not a precondition: without it the
      // select still offers None and the task's current milestone.
    }
  }

  // Saving sends the difference from the loaded draft, so a field the user left
  // alone is absent from the body and the server keeps it.
  async function saveEdit() {
    const patch = editPatch(draft, baseline);
    if (Object.keys(patch).length === 0) {
      closeEdit();
      return;
    }
    const guard = guardRef.current;
    setSaving(true);
    try {
      const view = await api.patchTask(taskId, patch);
      if (guard.cancelled) {
        return;
      }
      applyView(view);
      closeEdit();
      // The response carries the task alone, so a changed parent, milestone or
      // dependency needs the reload to move the chips and readiness with it.
      await load(guard);
    } catch (err) {
      if (!guard.cancelled) {
        setEditRejection(fieldError(err));
      }
    } finally {
      setSaving(false);
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
                draft=${draft}
                milestones=${milestones}
                editRejection=${editRejection}
                saving=${saving}
                onClose=${onClose}
                onSelect=${onSelect}
                onRun=${runAction}
                onToggle=${toggleCriterion}
                onEdit=${beginEdit}
                onDraftChange=${(field, value) =>
                  setDraft((current) => ({ ...current, [field]: value }))}
                onSave=${saveEdit}
                onCancelEdit=${closeEdit}
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
  draft,
  milestones,
  editRejection,
  saving,
  onClose,
  onSelect,
  onRun,
  onToggle,
  onEdit,
  onDraftChange,
  onSave,
  onCancelEdit,
}) {
  const { task, sections, criteria, criteria_heading } = context.detail;

  if (draft) {
    return html`
      <header class="drawer-header">
        <span class="drawer-id">${task.qualified_id}</span>
        <span class="drawer-badge">${task.status}</span>
        <span class="drawer-badge">${task.priority}</span>
        <button class="drawer-close" onClick=${onClose} title="Close">×</button>
      </header>
      <${DrawerEdit}
        draft=${draft}
        milestones=${milestones}
        rejection=${editRejection}
        saving=${saving}
        onChange=${onDraftChange}
        onSave=${onSave}
        onCancel=${onCancelEdit}
      />
    `;
  }

  return html`
    <header class="drawer-header">
      <span class="drawer-id">${task.qualified_id}</span>
      <span class="drawer-badge">${task.status}</span>
      <span class="drawer-badge">${task.priority}</span>
      <button class="drawer-close" onClick=${onClose} title="Close">×</button>
    </header>
    <h3 class="drawer-title">${task.title}</h3>
    <div class="drawer-action-row">
      <${DrawerActions} pendingAction=${pendingAction} onRun=${onRun} />
      <button class="drawer-edit-open" onClick=${onEdit}>Edit</button>
    </div>
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
      ${task.risk ? html`<dt>Risk</dt><dd>${task.risk}</dd>` : null}
      ${task.impact ? html`<dt>Impact</dt><dd>${task.impact}</dd>` : null}
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
