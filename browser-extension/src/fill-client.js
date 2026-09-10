import { DETECTION_STAGES, FILL_STAGES, PublicFailure, exact } from "./protocol.js";
import { fillLoginForm } from "./fill.js";

const DETECTION_WAIT_MS = 2500;
const DIAGNOSTIC_REASONS = new Set([
  "not_visible", "disabled", "readonly", "verification_code", "new_password",
  "password_candidate", "username_candidate", "unsupported_type", "non_login_metadata",
]);

function frameResult(value) {
  if (!value || !Number.isInteger(value.frameId) || value.frameId < 0
    || typeof value.documentId !== "string" || !value.documentId) throw new PublicFailure("PAGE_CHANGED");
  const result = value.result;
  if (exact(result, ["ok", "stage"]) && result.ok === true
    && DETECTION_STAGES.includes(result.stage) && result.stage !== "challenge") {
    return { frameId: value.frameId, documentId: value.documentId, stage: result.stage };
  }
  if (exact(result, ["ok", "stage", "kind"]) && result.ok === true
    && result.stage === "challenge" && ["otp", "website"].includes(result.kind)) {
    return { frameId: value.frameId, documentId: value.documentId, stage: result.stage, kind: result.kind };
  }
  if (exact(result, ["ok", "code"]) && result.ok === false
    && ["NO_LOGIN_FORM", "AMBIGUOUS_LOGIN_FORM", "PAGE_CHANGED"].includes(result.code)) {
    return { frameId: value.frameId, documentId: value.documentId, code: result.code };
  }
  throw new PublicFailure("NO_LOGIN_FORM");
}

function inspectionOf(results) {
  if (!Array.isArray(results) || results.length < 1 || results.length > 100) {
    throw new PublicFailure("PAGE_CHANGED");
  }
  const parsed = results.map(frameResult);
  const frameIds = new Set();
  const documentIds = new Set();
  for (const result of parsed) {
    if (frameIds.has(result.frameId) || documentIds.has(result.documentId)) {
      throw new PublicFailure("PAGE_CHANGED");
    }
    frameIds.add(result.frameId);
    documentIds.add(result.documentId);
  }
  const main = parsed.filter(result => result.frameId === 0);
  if (main.length !== 1) throw new PublicFailure("PAGE_CHANGED");
  if (main[0].code) throw new PublicFailure(main[0].code);
  const fillable = parsed.filter(result => !result.code && FILL_STAGES.includes(result.stage));
  if (parsed.some(result => result.stage === "ambiguous") || fillable.length > 1) {
    return { frameId: 0, documentId: main[0].documentId, stage: "ambiguous" };
  }
  if (fillable.length === 1) return fillable[0];
  const challenge = parsed.find(result => result.stage === "challenge" && result.kind === "website")
    ?? parsed.find(result => result.stage === "challenge");
  if (challenge) return challenge;
  const passwordless = parsed.find(result => result.stage === "passwordless");
  return passwordless ?? { frameId: 0, documentId: main[0].documentId, stage: "unsupported" };
}

function diagnosticText(value, max, allowEmpty = true) {
  return typeof value === "string" && (allowEmpty || value.length > 0) && value.length <= max
    && !/[\u0000-\u001f\u007f-\u009f]/.test(value);
}

function diagnosticFrame(value, expectedOrigin) {
  if (!value || !Number.isInteger(value.frameId) || value.frameId < 0
    || !diagnosticText(value.documentId, 256, false)) throw new PublicFailure("PAGE_CHANGED");
  const result = value.result;
  if (exact(result, ["ok", "code"]) && result.ok === false
    && ["NO_LOGIN_FORM", "AMBIGUOUS_LOGIN_FORM", "PAGE_CHANGED"].includes(result.code)) {
    return { frameId: value.frameId, documentId: value.documentId, status: "rejected", code: result.code };
  }
  if (!exact(result, ["ok", "stage", "kind", "diagnostics"]) || result.ok !== true
    || !DETECTION_STAGES.includes(result.stage)
    || (result.stage === "challenge" ? !["otp", "website"].includes(result.kind) : result.kind !== "")) {
    throw new PublicFailure("NO_LOGIN_FORM");
  }
  const diagnostic = result.diagnostics;
  if (!exact(diagnostic, ["origin", "counts", "fields", "truncated", "detectedStage"])
    || diagnostic.detectedStage !== result.stage || diagnostic.origin !== expectedOrigin
    || typeof diagnostic.truncated !== "boolean" || !Array.isArray(diagnostic.fields)
    || diagnostic.fields.length > 100) throw new PublicFailure("NO_LOGIN_FORM");
  const counts = diagnostic.counts;
  const countKeys = ["inputs", "visibleInputs", "usernameCandidates", "passwordCandidates",
    "otpCandidates", "interactiveControls", "challengeIndicators"];
  if (!exact(counts, countKeys) || !countKeys.every(key => Number.isInteger(counts[key])
    && counts[key] >= 0 && counts[key] <= 10000) || counts.visibleInputs > counts.inputs
    || counts.usernameCandidates > counts.inputs || counts.passwordCandidates > counts.inputs
    || counts.otpCandidates > counts.inputs || diagnostic.fields.length !== Math.min(counts.inputs, 100)
    || diagnostic.truncated !== (counts.inputs > 100)) throw new PublicFailure("NO_LOGIN_FORM");
  for (const [index, field] of diagnostic.fields.entries()) {
    if (!exact(field, ["index", "type", "autocomplete", "id", "name", "visible", "score", "rejectionReason"])
      || field.index !== index || !diagnosticText(field.type, 32, false)
      || !diagnosticText(field.autocomplete, 128) || !diagnosticText(field.id, 128)
      || !diagnosticText(field.name, 128) || typeof field.visible !== "boolean"
      || !Number.isSafeInteger(field.score) || field.score < 0 || field.score > 10000000
      || !DIAGNOSTIC_REASONS.has(field.rejectionReason)) throw new PublicFailure("NO_LOGIN_FORM");
  }
  return {
    frameId: value.frameId,
    documentId: value.documentId,
    status: "inspected",
    stage: result.stage,
    kind: result.kind,
    origin: diagnostic.origin,
    counts: { ...counts },
    fields: diagnostic.fields.map(field => ({ ...field })),
    truncated: diagnostic.truncated,
  };
}

function diagnosticInspectionOf(results, pageUrl) {
  if (!Array.isArray(results) || results.length < 1 || results.length > 100) {
    throw new PublicFailure("PAGE_CHANGED");
  }
  let expectedOrigin;
  try { expectedOrigin = new URL(pageUrl).origin; } catch { throw new PublicFailure("PAGE_CHANGED"); }
  const frames = results.map(value => diagnosticFrame(value, expectedOrigin));
  const compact = frames.map(frame => ({
    frameId: frame.frameId,
    documentId: frame.documentId,
    result: frame.status === "rejected"
      ? { ok: false, code: frame.code }
      : frame.stage === "challenge"
        ? { ok: true, stage: frame.stage, kind: frame.kind }
        : { ok: true, stage: frame.stage },
  }));
  const selected = inspectionOf(compact);
  return {
    version: 1,
    stage: selected.stage,
    kind: selected.kind ?? "",
    selectedFrameId: selected.frameId,
    selectedDocumentId: selected.documentId,
    frames,
  };
}

function injectedResult(results, expectedFrame, expectedDocument) {
  if (!Array.isArray(results) || results.length !== 1) throw new PublicFailure("PAGE_CHANGED");
  const result = frameResult(results[0]);
  if (result.frameId !== expectedFrame || result.documentId !== expectedDocument) {
    throw new PublicFailure("PAGE_CHANGED");
  }
  if (result.code) throw new PublicFailure(result.code);
  return result;
}

export async function inspectForm(api, page) {
  let results;
  try {
    results = await api.scripting.executeScript({
      target: { tabId: page.tabId, allFrames: true }, world: "ISOLATED", injectImmediately: true,
      func: fillLoginForm,
      args: [page.pageUrl, Date.now() + DETECTION_WAIT_MS + 1000, null, null, DETECTION_WAIT_MS],
    });
  } catch { throw new PublicFailure("NO_LOGIN_FORM"); }
  return inspectionOf(results);
}

// Explicit developer tool only. Production inspection never calls this path,
// and this function returns bounded structural metadata without input values.
export async function inspectFormDiagnostics(api, page) {
  let results;
  try {
    results = await api.scripting.executeScript({
      target: { tabId: page.tabId, allFrames: true }, world: "ISOLATED", injectImmediately: true,
      func: fillLoginForm,
      args: [page.pageUrl, Date.now() + 1000, null, null, 0, true],
    });
  } catch { throw new PublicFailure("NO_LOGIN_FORM"); }
  return diagnosticInspectionOf(results, page.pageUrl);
}

export async function injectFill(api, page, frameId, documentId, stage, payload) {
  let results;
  const args = [page.pageUrl, Date.now() + 1000, stage, payload];
  try {
    results = await api.scripting.executeScript({
      target: { tabId: page.tabId, documentIds: [documentId] }, world: "ISOLATED", injectImmediately: true,
      func: fillLoginForm, args,
    });
  } catch { throw new PublicFailure("PAGE_CHANGED"); }
  finally { args.length = 0; }
  const result = injectedResult(results, frameId, documentId);
  if (result.stage !== stage) throw new PublicFailure("PAGE_CHANGED");
}
