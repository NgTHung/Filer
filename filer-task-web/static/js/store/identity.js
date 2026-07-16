import { useEffect, useState } from "../../vendor/preact-htm.js";
import { getIdentity, putIdentity } from "../api/client.js";
const state = {
  identity: null,
  loading: true,
  error: null,
};
const listeners = new Set();
function notify() {
  for (const listener of listeners) {
    listener();
  }
}
export async function loadIdentity() {
  state.loading = true;
  state.error = null;
  notify();
  try {
    state.identity = await getIdentity();
  } catch (error) {
    if (error.code === "identity_required") {
      state.identity = null;
    } else {
      state.error = error;
    }
  } finally {
    state.loading = false;
    notify();
  }
  return state.identity;
}
export async function saveUsername(username) {
  const identity = await putIdentity(username);
  state.identity = identity;
  state.error = null;
  notify();
  return identity;
}
export function useIdentityStore() {
  const [, setTick] = useState(0);
  useEffect(() => {
    const listener = () => setTick((tick) => tick + 1);
    listeners.add(listener);
    return () => listeners.delete(listener);
  }, []);
  return state;
}
