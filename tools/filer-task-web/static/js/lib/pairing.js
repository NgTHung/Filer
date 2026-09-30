// Client-side guard for the pairing code field, mirroring the server's rule:
// exactly six ASCII digits. Trimming mirrors the server, which trims too, so a
// copied code with stray spaces still pairs.

export function validatePin(pin) {
  const trimmed = pin.trim();
  if (!/^\d{6}$/.test(trimmed)) {
    return { valid: false, error: "The pairing code is exactly 6 digits." };
  }
  return { valid: true, pin: trimmed };
}
