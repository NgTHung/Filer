import { html, useEffect, useState } from "../../vendor/preact-htm.js";
import { mintPairingPin } from "../api/client.js";
import { Dialog } from "./Dialog.js";

// Mints a six-digit code for signing into this identity on another browser.
// The code mints on open and can be replaced, because a mistyped or expired
// code is cheaper to throw away than to read.
export function PairBrowserDialog({ onCancel }) {
  const [pin, setPin] = useState(null);
  const [error, setError] = useState(null);

  async function mint() {
    setPin(null);
    setError(null);
    try {
      const pairing = await mintPairingPin();
      setPin(pairing.pin);
    } catch (requestError) {
      setError(requestError.message);
    }
  }

  useEffect(() => {
    mint();
  }, []);

  return html`
    <${Dialog} title="Pair another browser" onCancel=${onCancel}>
      <p class="muted-note">
        In the other browser choose "I already have a name" and enter your
        username with this code. The code expires in 5 minutes and works once.
      </p>
      ${error
        ? html`<p class="identity-error" role="alert">${error}</p>`
        : pin
          ? html`<p class="pairing-code" aria-label="Pairing code">${pin}</p>`
          : html`<p class="pairing-loading">Minting a code…</p>`}
      <div class="dialog-actions">
        <button type="button" onClick=${mint}>Mint a new code</button>
        <button type="button" onClick=${onCancel}>Close</button>
      </div>
    <//>
  `;
}
