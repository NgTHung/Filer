import { html, useState } from "../../vendor/preact-htm.js";
import { saveUsername, useIdentityStore } from "../store/identity.js";
export function IdentityFooter() {
  const { identity } = useIdentityStore();
  const [editing, setEditing] = useState(false);
  const [username, setUsername] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState(null);
  function beginEditing() {
    setUsername(identity.username);
    setError(null);
    setEditing(true);
  }
  async function submit(event) {
    event.preventDefault();
    setSaving(true);
    setError(null);
    try {
      await saveUsername(username);
      setEditing(false);
    } catch (requestError) {
      setError(requestError.message);
    } finally {
      setSaving(false);
    }
  }
  if (!editing) {
    return html`
      <button class="identity-footer" onClick=${beginEditing} title="Change username">
        <span class="identity-label">Signed in as</span>
        <span class="identity-name">${identity.username}</span>
      </button>
    `;
  }
  return html`
    <form class="identity-editor" onSubmit=${submit}>
      <label for="sidebar-username">Username</label>
      <input
        id="sidebar-username"
        value=${username}
        maxlength="64"
        autocomplete="name"
        disabled=${saving}
        onInput=${(event) => setUsername(event.currentTarget.value)}
      />
      ${error ? html`<p class="identity-error" role="alert">${error}</p>` : null}
      <div class="identity-actions">
        <button type="submit" disabled=${saving}>${saving ? "Saving…" : "Save"}</button>
        <button type="button" disabled=${saving} onClick=${() => setEditing(false)}>Cancel</button>
      </div>
    </form>
  `;
}
