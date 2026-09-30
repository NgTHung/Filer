import { html, useState } from "../../vendor/preact-htm.js";
import { validatePin } from "../lib/pairing.js";
import { saveUsername, pairIdentity } from "../store/identity.js";

// The two onboarding paths for a fresh browser: create a new name, or adopt an
// existing identity with a pairing code minted by a browser that already has it.
export function IdentityPrompt({ onComplete }) {
  const [mode, setMode] = useState("create");
  const [username, setUsername] = useState("");
  const [pin, setPin] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState(null);

  function switchMode(next) {
    setMode(next);
    setError(null);
  }

  async function submit(event) {
    event.preventDefault();
    const checked = mode === "pair" ? validatePin(pin) : { valid: true };
    if (!checked.valid) {
      setError(checked.error);
      return;
    }
    setSaving(true);
    setError(null);
    try {
      if (mode === "create") {
        await saveUsername(username);
      } else {
        await pairIdentity(username, checked.pin);
      }
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
        <h1>Welcome to Filer</h1>
        <p>Filer uses your name to attribute changes.</p>
        <div class="identity-modes" role="tablist" aria-label="Identity">
          <button
            type="button"
            role="tab"
            aria-selected=${mode === "create"}
            class=${mode === "create" ? "identity-mode-selected" : ""}
            onClick=${() => switchMode("create")}
          >
            Create a name
          </button>
          <button
            type="button"
            role="tab"
            aria-selected=${mode === "pair"}
            class=${mode === "pair" ? "identity-mode-selected" : ""}
            onClick=${() => switchMode("pair")}
          >
            I already have a name
          </button>
        </div>
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
        ${mode === "pair"
          ? html`
              <label for="identity-pin">Pairing code</label>
              <input
                id="identity-pin"
                name="pin"
                value=${pin}
                inputmode="numeric"
                maxlength="6"
                autocomplete="one-time-code"
                disabled=${saving}
                onInput=${(event) => setPin(event.currentTarget.value)}
              />
              <p class="muted-note">
                Ask the browser already signed in as this name for a code
                (Footer, "Pair another browser").
              </p>
            `
          : null}
        ${error ? html`<p class="identity-error" role="alert">${error}</p>` : null}
        <button type="submit" disabled=${saving}>${saving ? "Saving…" : "Continue"}</button>
      </form>
    </main>
  `;
}
