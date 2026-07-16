import { html, useState } from "../../vendor/preact-htm.js";
import { saveUsername } from "../store/identity.js";
export function IdentityPrompt({ onComplete }) {
  const [username, setUsername] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState(null);
  async function submit(event) {
    event.preventDefault();
    setSaving(true);
    setError(null);
    try {
      await saveUsername(username);
      onComplete();
    } catch (requestError) {
      setError(requestError.message);
    } finally {
      setSaving(false);
    }
  }
  return html`
    <main class="identity-onboarding">
      <form class="identity-card" onSubmit=${submit}>
        <h1>Choose your name</h1>
        <p>Filer uses this name to attribute changes.</p>
        <label for="identity-username">Username</label>
        <input
          id="identity-username"
          name="username"
          value=${username}
          maxlength="64"
          autocomplete="name"
          autofocus
          disabled=${saving}
          onInput=${(event) => setUsername(event.currentTarget.value)}
        />
        ${error ? html`<p class="identity-error" role="alert">${error}</p>` : null}
        <button type="submit" disabled=${saving}>${saving ? "Saving…" : "Continue"}</button>
      </form>
    </main>
  `;
}
