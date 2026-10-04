import { html, useEffect, useMemo, useState } from "../../vendor/preact-htm.js";
import { projectScoped } from "../api/client.js";
import { isLastOwner } from "../lib/access.js";

const LAST_OWNER_NOTE = "A project needs at least one owner";

function formatTime(unixSeconds) {
  return new Date(unixSeconds * 1000).toLocaleString();
}

// Members of one project. Owners change roles and remove members here. The
// last owner's controls stay disabled, because the server refuses to leave a
// project without an owner.
export function MemberList({ projectName, canManage, reloadKey }) {
  const api = useMemo(() => projectScoped(projectName), [projectName]);
  const [members, setMembers] = useState([]);
  const [error, setError] = useState(null);
  const [busyId, setBusyId] = useState(null);

  async function load() {
    try {
      const response = await api.listMembers();
      setMembers(response.members);
      setError(null);
    } catch (requestError) {
      setError(requestError);
    }
  }

  useEffect(() => {
    load();
  }, [projectName, reloadKey]);

  async function run(member, change) {
    setBusyId(member.user_id);
    setError(null);
    try {
      await change();
      await load();
    } catch (requestError) {
      setError(requestError);
    } finally {
      setBusyId(null);
    }
  }

  function remove(member) {
    if (!confirm(`Remove ${member.username} from ${projectName}?`)) {
      return;
    }
    run(member, () => api.removeMember(member.user_id));
  }

  return html`
    <div class="member-list">
      ${error
        ? html`<p class="screen-error" role="alert">Member request failed: ${error.message}</p>`
        : null}
      ${members.length === 0 && !error ? html`<p class="empty-state">No members.</p>` : null}
      ${members.map((member) => {
        const locked = isLastOwner(member, members);
        const busy = busyId === member.user_id;
        return html`
          <div class="session-row" key=${member.user_id}>
            <div class="session-label">
              <span>${member.username}</span>
            </div>
            <div class="session-meta">
              <span>${member.granted_by ? `Granted by ${member.granted_by}` : "Granted by the server"}</span>
              <span>${formatTime(member.granted_at)}</span>
            </div>
            ${canManage
              ? html`
                  <select
                    class="member-role"
                    value=${member.role}
                    disabled=${locked || busy}
                    title=${locked ? LAST_OWNER_NOTE : ""}
                    onChange=${(event) =>
                      run(member, () => api.setMemberRole(member.user_id, event.currentTarget.value))}
                  >
                    <option value="owner">Owner</option>
                    <option value="editor">Editor</option>
                  </select>
                  <button
                    type="button"
                    class="session-revoke"
                    disabled=${locked || busy}
                    title=${locked ? LAST_OWNER_NOTE : ""}
                    onClick=${() => remove(member)}
                  >
                    ${busy ? "Working…" : "Remove"}
                  </button>
                `
              : html`<span class="session-current">${member.role}</span>`}
          </div>
        `;
      })}
    </div>
  `;
}
