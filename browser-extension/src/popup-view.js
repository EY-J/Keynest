const messages = {
  loading: ["Checking KeyNest...", ""],
  locked: ["KeyNest is locked.", "Unlock KeyNest to review this login host."],
  unavailable: ["KeyNest desktop app is unavailable.", "Open KeyNest and unlock your vault, then try again. If this continues, check the Autofill setup."],
  unsupported_url: ["This page is not supported.", "Open an HTTPS website, then open KeyNest Autofill again."],
  unsupported: ["Autofill is unavailable on this device.", "KeyNest Autofill currently supports Windows with Chrome or Edge."],
  error: ["KeyNest could not complete this request.", "Try again from your website."],
  no_matches: ["No approved login is available for:", "If you already have a KeyNest credential for this account, you can approve this exact login host in KeyNest."],
  approval_requested: ["Review requested in KeyNest.", "Approve the exact login host in the desktop app, then return here and click Retry."],
  matches: ["Saved logins for:", ""],
  no_login_form: ["No supported login form found.", "Use a visible username and/or password field, then try again."],
  ambiguous_login_form: ["More than one login field or form could match.", "Open a page with one clear login form, then try again."],
  challenge: ["Verification-code step detected.", "KeyNest does not fill one-time codes yet."],
  security_challenge: ["Complete the website's verification step, then try KeyNest again.", ""],
  passwordless: ["Passwordless/passkey sign-in detected.", "Use the website's sign-in flow, or choose password sign-in manually if offered."],
  domain_mismatch: ["This login does not match the current website.", "Refresh the saved logins and check the website."],
  credential_not_found: ["This saved login is no longer available.", "Refresh the saved logins and try again."],
  page_changed: ["The active page changed.", "Open KeyNest Autofill again on the website you want to use."],
};

export function renderView(document, view, onFill, onReview) {
  const action = ({ combined: "login", username_only: "username", password_only: "password" })[view.stage];
  const [message, hint] = view.state === "filling"
    ? [`Filling your ${action || "login"}...`, ""]
    : view.state === "filled"
      ? [`${action === "login" ? "Login" : action === "username" ? "Username" : "Password"} filled.`, "Review the website, then continue or submit it yourself."]
      : messages[view.state] ?? messages.error;
  const busy = view.state === "loading" || view.state === "filling";
  document.getElementById("content").setAttribute("aria-busy", String(busy));
  document.getElementById("status").textContent = message;
  const host = document.getElementById("host");
  const hostValue = document.getElementById("host-value") ?? host;
  hostValue.textContent = view.host || "";
  host.hidden = !view.host;
  document.getElementById("host-group")?.setAttribute("data-state", view.state);
  document.getElementById("hint").textContent = hint;
  document.getElementById("retry").disabled = busy;
  const review = document.getElementById("review");
  review.hidden = !["no_matches", "locked"].includes(view.state);
  review.disabled = busy || !onReview;
  review.onclick = onReview ? event => { if (event.isTrusted) onReview(); } : null;
  const list = document.getElementById("matches");
  list.replaceChildren();
  if (view.state !== "matches") return;
  for (const match of view.matches) {
    const row = document.createElement("li");
    const account = document.createElement("div");
    account.className = "account";
    const name = document.createElement("strong");
    name.className = "account-name";
    name.textContent = match.name;
    const username = document.createElement("span");
    username.className = "account-user";
    username.textContent = match.username;
    const fill = document.createElement("button");
    fill.type = "button";
    fill.textContent = `Fill ${action}`;
    fill.disabled = !onFill;
    fill.setAttribute("aria-label", `Fill ${action} for ${match.name}`);
    // Only the nonsecret ID is retained in the click handler.
    const selectedId = match.credentialId;
    if (onFill) fill.addEventListener("click", event => { if (event.isTrusted) onFill(selectedId); });
    account.append(name, username);
    row.append(account, fill);
    list.append(row);
  }
}
