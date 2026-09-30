import { html, useState } from "../../vendor/preact-htm.js";
import { domainNames, prefixesFor } from "../lib/policy.js";
import {
  addDomainOperation,
  addPrefixOperation,
  removeDomainOperation,
  removePrefixOperation,
} from "../lib/policyOps.js";
import { sectionRejection } from "../lib/policyRejection.js";
import { RejectionNotice } from "./RejectionNotice.js";

export function PolicyDomains({ policy, rejection, armed, onArm, onSubmit, busy }) {
  const [name, setName] = useState("");
  const [prefixes, setPrefixes] = useState("");
  const [drafts, setDrafts] = useState({});

  const added = addDomainOperation(name, prefixes);

  function submitDomain(event) {
    event.preventDefault();
    onSubmit(added).then((accepted) => {
      if (accepted) {
        setName("");
        setPrefixes("");
      }
    });
  }

  function submitPrefix(event, domain) {
    event.preventDefault();
    onSubmit(addPrefixOperation(domain, drafts[domain] ?? "")).then((accepted) => {
      if (accepted) {
        setDrafts((current) => ({ ...current, [domain]: "" }));
      }
    });
  }

  return html`
    <section class="policy-section">
      <h3 class="settings-heading">Domains</h3>
      <${RejectionNotice} rejection=${sectionRejection(rejection, "domains")} />
      ${domainNames(policy).map(
        (domain) => html`
          <div class="policy-row" key=${domain}>
            <div class="policy-row-head">
              <span class="policy-name">${domain}</span>
              <button
                type="button"
                disabled=${busy}
                onClick=${() =>
                  armed === domain ? onSubmit(removeDomainOperation(domain)) : onArm(domain)}
              >
                ${armed === domain ? "Confirm remove" : "Remove"}
              </button>
            </div>
            <div class="chip-row">
              ${prefixesFor(policy, domain).map(
                (prefix) => html`
                  <span class="policy-chip" key=${prefix}>
                    ${prefix}
                    <button
                      type="button"
                      class="chip-remove"
                      title=${`Remove prefix ${prefix}`}
                      disabled=${busy}
                      onClick=${() => onSubmit(removePrefixOperation(domain, prefix))}
                    >
                      ×
                    </button>
                  </span>
                `,
              )}
            </div>
            <form class="policy-add" onSubmit=${(event) => submitPrefix(event, domain)}>
              <input
                type="text"
                placeholder="new prefix"
                value=${drafts[domain] ?? ""}
                onInput=${(event) => {
                  const value = event.currentTarget.value;
                  setDrafts((current) => ({ ...current, [domain]: value }));
                }}
              />
              <button
                type="submit"
                disabled=${!addPrefixOperation(domain, drafts[domain] ?? "") || busy}
              >
                Add prefix
              </button>
            </form>
            <${RejectionNotice} rejection=${sectionRejection(rejection, `domain:${domain}`)} />
          </div>
        `,
      )}
      <form class="policy-add" onSubmit=${submitDomain}>
        <input
          type="text"
          placeholder="new domain"
          value=${name}
          onInput=${(event) => setName(event.currentTarget.value)}
        />
        <input
          type="text"
          placeholder="its prefixes, comma separated"
          value=${prefixes}
          onInput=${(event) => setPrefixes(event.currentTarget.value)}
        />
        <button type="submit" disabled=${!added || busy}>Add domain</button>
      </form>
    </section>
  `;
}
