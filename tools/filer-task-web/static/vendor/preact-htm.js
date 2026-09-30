// Vendored: Preact 10.19.3 (preact.module.js, hooks.module.js) + htm 3.1.1 (htm.module.js).
// Re-exports the pieces the app modules need, with `html` pre-bound to Preact's `h`.
import { h, render, Fragment } from "./preact.module.js";
import { useState, useEffect, useMemo, useCallback, useRef } from "./hooks.module.js";
import htm from "./htm.module.js";

export const html = htm.bind(h);
export { render, Fragment, useState, useEffect, useMemo, useCallback, useRef };
