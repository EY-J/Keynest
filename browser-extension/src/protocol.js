export const HOST_NAME = "com.eyy.keynest.autofill";
export const POPUP_PORT = "keynest-autofill-popup-v1";
export const MAX_BYTES = 65536;
export const DETECTION_STAGES = ["combined", "username_only", "password_only", "unsupported", "ambiguous", "challenge", "passwordless"];
export const FILL_STAGES = ["combined", "username_only", "password_only"];
const encoder = new TextEncoder();
const codes = new Set([
  "APP_NOT_RUNNING", "VAULT_LOCKED", "UNSUPPORTED_URL", "NO_MATCHES",
  "CREDENTIAL_NOT_FOUND", "DOMAIN_MISMATCH", "INVALID_REQUEST", "INTERNAL_ERROR",
  "NATIVE_HOST_UNAVAILABLE", "IPC_UNAVAILABLE", "PAGE_CHANGED",
  "NO_LOGIN_FORM", "AMBIGUOUS_LOGIN_FORM",
]);

export class PublicFailure extends Error {
  constructor(code) {
    super("KeyNest Autofill request failed.");
    this.code = codes.has(code) ? code : "INTERNAL_ERROR";
  }
}

export function exact(value, keys) {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    && Object.keys(value).length === keys.length
    && keys.every(key => Object.hasOwn(value, key));
}

export function requestId(value) {
  return typeof value === "string" && /^[A-Za-z0-9_-]{1,128}$/.test(value);
}

function boundedText(value, max) {
  if (typeof value !== "string" || value.length === 0) return false;
  let count = 0;
  for (const character of value) { if (++count > max) return false; }
  return true;
}

export function credentialId(value) { return typeof value === "string" && /^[a-f0-9]{32}$/.test(value); }

// Drop references only. This does not securely erase JavaScript strings.
export function releaseFill(payload) {
  if (payload && typeof payload === "object") { payload.username = ""; payload.password = ""; }
}

export function summaries(value) {
  if (!Array.isArray(value) || value.length > 1024) return false;
  const ids = new Set();
  return value.every(item => {
    if (!exact(item, ["credentialId", "name", "username"])
      || typeof item.credentialId !== "string" || !/^[a-f0-9]{32}$/.test(item.credentialId)
      || ids.has(item.credentialId) || !boundedText(item.name, 200)
      || !boundedText(item.username, 500)) return false;
    ids.add(item.credentialId);
    return true;
  });
}

// Eligibility only; the Rust vault independently performs authoritative matching.
export function eligiblePage(value) {
  if (typeof value !== "string" || encoder.encode(value).length > 8192
    || /[\s\u0000-\u001f\u007f-\u009f\\]/u.test(value) || !/^https:\/\/[^/]/i.test(value)) {
    throw new PublicFailure("UNSUPPORTED_URL");
  }
  let url;
  try { url = new URL(value); } catch { throw new PublicFailure("UNSUPPORTED_URL"); }
  const host = url.hostname.toLowerCase().replace(/\.$/, "");
  if (url.protocol !== "https:" || !host || url.username || url.password
    || value.slice(value.indexOf("://") + 3).split(/[/?#]/, 1)[0].includes("@")
    || host.split(".").some(label => !label)) throw new PublicFailure("UNSUPPORTED_URL");
  return { pageUrl: url.href, host };
}

export function validateNativeResponse(value, request) {
  const invalid = () => { throw new PublicFailure("IPC_UNAVAILABLE"); };
  if (!value || value.version !== 1 || value.requestId !== request.requestId) invalid();
  if (value.ok === false) {
    if (!exact(value, ["version", "requestId", "ok", "type", "error"])
      || value.type !== "error" || !exact(value.error, ["code", "message"])
      || !codes.has(value.error.code) || typeof value.error.message !== "string"
      || value.error.message.length > 1024) invalid();
    // Ignore all native error text. Only fixed, local UI text is ever displayed.
    throw new PublicFailure(value.error.code);
  }
  if (value.ok !== true || !exact(value, ["version", "requestId", "ok", "type", "data"])) invalid();
  if (request.type === "status") {
    if (value.type !== "status" || !exact(value.data, ["state"])
      || !["unlocked", "locked", "app_not_running", "unsupported", "error"].includes(value.data.state)) invalid();
  } else if (request.type === "queryMatches") {
    if (value.type !== "matches" || !exact(value.data, ["matches"]) || !summaries(value.data.matches)) invalid();
  } else if (request.type === "requestFill") {
    if (value.type !== "fillPayload" || !exact(value.data, ["username", "password"])
      || !boundedText(value.data.username, 500) || !boundedText(value.data.password, 4096)) invalid();
    // These field bounds guarantee < 64 KiB even with maximal JSON escaping.
    // Avoid creating another serialized secret string merely to count bytes.
    return { username: value.data.username, password: value.data.password };
  } else if (request.type === "requestHostApproval") {
    if (value.type !== "hostApprovalRequested" || !exact(value.data, ["state"])
      || value.data.state !== "requested") invalid();
  } else invalid();
  if (encoder.encode(JSON.stringify(value)).length > MAX_BYTES) invalid();
  return value.data;
}

export function errorState(error) {
  switch (error instanceof PublicFailure ? error.code : "INTERNAL_ERROR") {
    case "VAULT_LOCKED": return "locked";
    case "APP_NOT_RUNNING": case "NATIVE_HOST_UNAVAILABLE": case "IPC_UNAVAILABLE": return "unavailable";
    case "UNSUPPORTED_URL": return "unsupported_url";
    case "NO_MATCHES": return "no_matches";
    case "PAGE_CHANGED": return "page_changed";
    case "NO_LOGIN_FORM": return "no_login_form";
    case "AMBIGUOUS_LOGIN_FORM": return "ambiguous_login_form";
    case "DOMAIN_MISMATCH": return "domain_mismatch";
    case "CREDENTIAL_NOT_FOUND": return "credential_not_found";
    default: return "error";
  }
}

export const VIEW_STATES = ["loading", "filling", "filled", "locked", "unavailable", "unsupported_url", "unsupported", "error", "no_matches", "approval_requested", "matches", "page_changed", "no_login_form", "ambiguous_login_form", "challenge", "security_challenge", "passwordless", "domain_mismatch", "credential_not_found"];

export function validView(value) {
  const stageIsValid = value && ((["matches", "filling", "filled"].includes(value.state) && FILL_STAGES.includes(value.stage))
    || (value.state === "no_login_form" && value.stage === "unsupported")
    || (value.state === "ambiguous_login_form" && value.stage === "ambiguous")
    || (["challenge", "security_challenge"].includes(value.state) && value.stage === "challenge")
    || (value.state === "passwordless" && value.stage === "passwordless")
    || (!["matches", "filling", "filled", "no_login_form", "ambiguous_login_form", "challenge", "security_challenge", "passwordless"].includes(value.state)
      && value.stage === ""));
  return exact(value, ["version", "requestId", "type", "state", "stage", "host", "matches"])
    && value.version === 1 && requestId(value.requestId) && value.type === "view"
    && VIEW_STATES.includes(value.state) && stageIsValid
    && typeof value.host === "string" && value.host.length <= 8192
    && summaries(value.matches) && (value.state === "matches" ? value.matches.length > 0 : value.matches.length === 0)
    && encoder.encode(JSON.stringify(value)).length <= MAX_BYTES;
}
