import assert from "node:assert/strict";
import { test } from "node:test";

import { validatePin } from "../../static/js/lib/pairing.js";

test("a six-digit pin is accepted and trimmed", () => {
  assert.deepEqual(validatePin("123456"), { valid: true, pin: "123456" });
  assert.deepEqual(validatePin(" 123456 "), { valid: true, pin: "123456" });
});

test("shorter, longer, and non-digit pins are rejected", () => {
  for (const pin of ["", "12", "12345", "1234567", "abcdef", "12 456", "123456a"]) {
    assert.equal(validatePin(pin).valid, false, `expected ${pin} to be rejected`);
  }
});
