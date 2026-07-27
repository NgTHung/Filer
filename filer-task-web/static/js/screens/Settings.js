import { html, useEffect, useMemo, useRef, useState } from "../../vendor/preact-htm.js";
import { ApiError, projectScoped, registerProject } from "../api/client.js";
import { Header } from "../components/Header.js";
import { PolicyDomains } from "../components/PolicyDomains.js";
import { PolicyTags } from "../components/PolicyTags.js";
import { PolicyTaskTypes } from "../components/PolicyTaskTypes.js";
import { ProjectOpenForm } from "../components/ProjectOpenForm.js";
import { RejectionNotice } from "../components/RejectionNotice.js";
import { sectionForOperation } from "../lib/policyOps.js";
import { policyRejection, sectionRejection } from "../lib/policyRejection.js";
import { loadProjects, setActiveProject } from "../store/project.js";

export function SettingsScreen({ projectName }) {
  const api = useMemo(() => (projectName ? projectScoped(projectName) : null), [projectName]);
  const [policy, setPolicy] = useState(null);
  const [rejection, setRejection] = useState(null);
  const [busy, setBusy] = useState(false);
  const [armed, setArmed] = useState(null);
  // A response that outlives a project switch would repaint this screen with
  // the policy of the project the user has already left.
  const guardRef = useRef({ cancelled: false });

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

  useEffect(() => {
    const guard = { cancelled: false };
    guardRef.current = guard;
    setArmed(null);
    load(guard);
    return () => {
      guard.cancelled = true;
    };
  }, [projectName]);

  async function open(path, init) {
    setBusy(true);
    setRejection(null);
    try {
      const summary = await registerProject(path, init);
      await activate(summary.name);
      return true;
    } catch (error) {
      // A path that resolves to an already-registered project is the "find"
      // half of find-or-create, so it switches instead of reporting a failure.
      if (error instanceof ApiError && error.code === "duplicate_project_name" && error.project) {
        await activate(error.project);
        return true;
      }
      setRejection({ section: "project", ...policyRejection(error) });
      return false;
    } finally {
      setBusy(false);
    }
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
      <${Header} title="Settings" onRefresh=${load} />
      <h3 class="settings-heading">Open a project</h3>
      <${ProjectOpenForm}
        onOpen=${open}
        busy=${busy}
        rejection=${sectionRejection(rejection, "project")}
      />
      <${RejectionNotice} rejection=${sectionRejection(rejection, "policy")} />
      ${policy
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
    </section>
  `;
}

// loadProjects only re-picks a default when the active name has disappeared, so
// the switch to a freshly opened project has to be explicit.
async function activate(name) {
  await loadProjects();
  setActiveProject(name);
}
