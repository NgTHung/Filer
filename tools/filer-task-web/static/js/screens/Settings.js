import { html, useEffect, useMemo, useRef, useState } from "../../vendor/preact-htm.js";
import { listSessions, projectScoped } from "../api/client.js";
import { AccessRequests } from "../components/AccessRequests.js";
import { Header } from "../components/Header.js";
import { MemberList } from "../components/MemberList.js";
import { PolicyDomains } from "../components/PolicyDomains.js";
import { PolicyTags } from "../components/PolicyTags.js";
import { PolicyTaskTypes } from "../components/PolicyTaskTypes.js";
import { ProjectCreateDialog } from "../components/ProjectCreateDialog.js";
import { ProjectOpenDialog } from "../components/ProjectOpenDialog.js";
import { RejectionNotice } from "../components/RejectionNotice.js";
import { SessionList } from "../components/SessionList.js";
import { ACTIONS, can } from "../lib/access.js";
import { sectionForOperation } from "../lib/policyOps.js";
import { policyRejection, sectionRejection } from "../lib/policyRejection.js";
import { openProject } from "../lib/projectOpen.js";
import { fieldError } from "../lib/rejection.js";
import { sessionRequestFailed, sessionRequestSucceeded } from "../lib/sessions.js";
import { useIdentityStore } from "../store/identity.js";
import { activeProject, loadProjects, useProjectStore } from "../store/project.js";

export function SettingsScreen({ projectName }) {
  const identityStore = useIdentityStore();
  useProjectStore();
  const api = useMemo(() => (projectName ? projectScoped(projectName) : null), [projectName]);
  const [policy, setPolicy] = useState(null);
  const [rejection, setRejection] = useState(null);
  const [busy, setBusy] = useState(false);
  const [sessionState, setSessionState] = useState(sessionRequestSucceeded([]));
  const [sessionBusy, setSessionBusy] = useState(false);
  const [armed, setArmed] = useState(null);
  const [dialog, setDialog] = useState(null);
  const [memberReload, setMemberReload] = useState(0);
  // A response that outlives a project switch would repaint this screen with
  // the policy of the project the user has already left.
  const guardRef = useRef({ cancelled: false });

  const project = activeProject();
  const role = project && project.name === projectName ? project.my_role ?? null : null;
  const isAdmin = identityStore.identity?.is_admin === true;
  const canRegister = can(role, isAdmin, ACTIONS.REGISTER_PROJECTS);
  const canEditPolicy = can(role, isAdmin, ACTIONS.EDIT_POLICY);
  const canManageMembers = can(role, isAdmin, ACTIONS.MANAGE_MEMBERS);

  async function load(guard = guardRef.current) {
    if (!api) {
      setPolicy(null);
      return;
    }
    try {
      const loaded = await api.getPolicy();
      if (!guard.cancelled) {
        setPolicy(loaded);
        setRejection(null);
      }
    } catch (error) {
      if (!guard.cancelled) {
        setPolicy(null);
        setRejection({ section: "policy", ...policyRejection(error) });
      }
    }
  }

  async function loadSessions(guard = guardRef.current) {
    setSessionBusy(true);
    try {
      const response = await listSessions();
      if (!guard.cancelled) {
        setSessionState(sessionRequestSucceeded(response.sessions));
      }
    } catch (error) {
      if (!guard.cancelled) {
        setSessionState(sessionRequestFailed([], error));
      }
    } finally {
      if (!guard.cancelled) {
        setSessionBusy(false);
      }
    }
  }

  async function refresh() {
    setMemberReload((count) => count + 1);
    await Promise.all([load(), loadSessions()]);
  }

  useEffect(() => {
    const guard = { cancelled: false };
    guardRef.current = guard;
    setArmed(null);
    load(guard);
    loadSessions(guard);
    return () => {
      guard.cancelled = true;
    };
  }, [projectName]);

  // Both dialogs report their own refusal, so the screen only has to close them
  // once the project they asked for is the active one.
  async function open(path) {
    return dismissOnSuccess(await openProject(path, false), policyRejection);
  }

  async function create(location, name) {
    return dismissOnSuccess(await openProject(location, true, name), fieldError);
  }

  function dismissOnSuccess(result, normalize) {
    if (result.ok) {
      setDialog(null);
      return { ok: true };
    }
    return { ok: false, rejection: normalize(result.error) };
  }

  // The response carries the whole refreshed policy, so an accepted change
  // needs no second read. Project summaries do go stale, because a domain
  // change moves the sidebar's counts.
  async function submit(operation) {
    if (!operation || !api) {
      return false;
    }
    const guard = guardRef.current;
    setBusy(true);
    setRejection(null);
    setArmed(null);
    try {
      const fresh = await api.patchPolicy(operation);
      if (!guard.cancelled) {
        setPolicy(fresh);
      }
      await loadProjects();
      return true;
    } catch (error) {
      if (!guard.cancelled) {
        setRejection({ section: sectionForOperation(operation), ...policyRejection(error) });
      }
      return false;
    } finally {
      setBusy(false);
    }
  }

  return html`
    <section class="screen">
      <${Header} title="Settings" onRefresh=${refresh} />
      <h3 class="settings-heading">Projects</h3>
      ${canRegister
        ? html`
            <div class="settings-actions">
              <button type="button" onClick=${() => setDialog("open")}>Open a project…</button>
              <button type="button" onClick=${() => setDialog("create")}>Create a project…</button>
            </div>
          `
        : html`<p class="muted-note">Only server admins can register projects.</p>`}
      <${RejectionNotice} rejection=${sectionRejection(rejection, "policy")} />
      ${policy && canEditPolicy
        ? html`
            <${PolicyDomains}
              policy=${policy}
              rejection=${rejection}
              armed=${armed}
              onArm=${setArmed}
              onSubmit=${submit}
              busy=${busy}
            />
            <${PolicyTaskTypes}
              policy=${policy}
              rejection=${rejection}
              onSubmit=${submit}
              busy=${busy}
            />
            <${PolicyTags} policy=${policy} rejection=${rejection} onSubmit=${submit} busy=${busy} />
          `
        : null}
      ${policy && !canEditPolicy
        ? html`<p class="muted-note">Only project owners can change domains, task types, and tags.</p>`
        : null}
      ${projectName
        ? html`
            <h3 class="settings-heading">Members</h3>
            <${MemberList}
              projectName=${projectName}
              canManage=${canManageMembers}
              reloadKey=${memberReload}
            />
          `
        : null}
      ${projectName && canManageMembers
        ? html`
            <h3 class="settings-heading">Access requests</h3>
            <${AccessRequests}
              projectName=${projectName}
              reloadKey=${memberReload}
              onApproved=${() => setMemberReload((count) => count + 1)}
            />
          `
        : null}
      <h3 class="settings-heading">Active sessions</h3>
      <${SessionList}
        sessions=${sessionState.sessions}
        error=${sessionState.error}
        busy=${sessionBusy}
        onRevoked=${loadSessions}
        onError=${(error) =>
          setSessionState((current) =>
            error
              ? sessionRequestFailed(current.sessions, error)
              : sessionRequestSucceeded(current.sessions),
          )}
      />
      ${dialog === "open"
        ? html`<${ProjectOpenDialog} onOpen=${open} onCancel=${() => setDialog(null)} />`
        : null}
      ${dialog === "create"
        ? html`<${ProjectCreateDialog} onCreate=${create} onCancel=${() => setDialog(null)} />`
        : null}
    </section>
  `;
}
