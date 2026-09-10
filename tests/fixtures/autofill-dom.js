import { fillLoginForm } from "../../browser-extension/src/fill.js";

// Real browser layout, input prototypes, forms and events. Only the window URL
// is substituted to test HTTPS guards from an offline file fixture. Function
// construction is confined to this test fixture, never extension-owned code.
const invoke = new Function("window", "expected", "expires", "stage", "payload", "waitMs", "diagnosticMode", `return (${fillLoginForm.toString()})(expected, expires, stage, payload, waitMs, diagnosticMode);`);
const expected = "https://fixture.test/login";
const fixture = document.getElementById("fixture");
const results = [];
const fakePassword = "FAKE-DOM-only-password";
function context() {
  const value = { location: { protocol: "https:", href: expected, origin: "https://fixture.test" } };
  value.top = value; value.parent = value; return value;
}
function assert(condition) { if (!condition) throw new Error("DOM assertion failed"); }
function test(name, action) {
  fixture.replaceChildren();
  try { action(); results.push({ name, ok: true }); }
  catch { results.push({ name, ok: false }); }
  finally { fixture.replaceChildren(); }
}
async function asyncTest(name, action) {
  fixture.replaceChildren();
  try { await action(); results.push({ name, ok: true }); }
  catch { results.push({ name, ok: false }); }
  finally { fixture.replaceChildren(); }
}
function input(parent, type, properties = {}) {
  const element = document.createElement("input"); element.type = type;
  Object.assign(element, properties); parent.append(element); return element;
}
function form() { const element = document.createElement("form"); fixture.append(element); return element; }
function pair(type = "email", parent = form()) { return { parent, user: input(parent, type), password: input(parent, "password") }; }
function inspect(window = context(), url = expected, expires = Date.now() + 1000) {
  return invoke(window, url, expires, null, null);
}
function diagnose(window = context(), url = expected, expires = Date.now() + 1000) {
  return invoke(window, url, expires, null, null, 0, true);
}
function fill(window = context(), url = expected, expires = Date.now() + 1000, stage = "combined") {
  const payload = { username: "fake-user", password: fakePassword };
  const result = invoke(window, url, expires, stage, payload);
  assert(payload.username === "" && payload.password === "");
  return result;
}
function success(result, stage = "combined", kind = null) {
  assert(JSON.stringify(result) === JSON.stringify(kind === null ? { ok: true, stage } : { ok: true, stage, kind }));
}
function failure(result, code) { assert(JSON.stringify(result) === JSON.stringify({ ok: false, code })); }

test("email/password fill with exact values", () => {
  const fields = pair(); success(fill()); assert(fields.user.value === "fake-user" && fields.password.value === fakePassword);
});
test("text username/password fill", () => {
  const fields = pair("text"); success(fill()); assert(fields.user.value === "fake-user" && fields.password.value === fakePassword);
});
test("username-only email, text and tel stages fill only the dominant username field", () => {
  for (const [type, properties] of [["email", {}], ["text", { name: "username" }], ["tel", { name: "mobile" }]]) {
    fixture.replaceChildren(); const parent = form(); const user = input(parent, type, properties);
    success(inspect(), "username_only"); success(fill(context(), expected, Date.now() + 1000, "username_only"), "username_only");
    assert(user.value === "fake-user");
  }
});
test("password-only stage fills only one usable password", () => {
  const parent = form(); const password = input(parent, "password");
  success(inspect(), "password_only"); success(fill(context(), expected, Date.now() + 1000, "password_only"), "password_only");
  assert(password.value === fakePassword);
});
test("search-only is unsupported and OTP-only is a challenge", () => {
  input(form(), "search", { name: "site-search" }); success(inspect(), "unsupported");
  fixture.replaceChildren(); input(form(), "text", { name: "otp", autocomplete: "one-time-code" }); success(inspect(), "challenge", "otp");
});
test("generic six-digit numeric input is a challenge and cannot receive credentials", () => {
  const code = input(form(), "text", { inputMode: "numeric", maxLength: 6 });
  success(inspect(), "challenge", "otp"); failure(fill(), "NO_LOGIN_FORM"); assert(code.value === "");
});
test("grouped one-character inputs are a challenge", () => {
  const parent = form(); const boxes = [];
  for (let index = 0; index < 6; index++) boxes.push(input(parent, "text", { inputMode: "numeric", maxLength: 1 }));
  success(inspect(), "challenge", "otp"); failure(fill(), "NO_LOGIN_FORM");
  assert(boxes.every(box => box.value === ""));
});
test("a distinguishable password remains fillable beside an OTP password box", () => {
  const parent = form();
  const password = input(parent, "password", { name: "password", autocomplete: "current-password" });
  const code = input(parent, "password", { name: "verification_code", inputMode: "numeric", maxLength: 6 });
  success(inspect(), "password_only");
  success(fill(context(), expected, Date.now() + 1000, "password_only"), "password_only");
  assert(password.value === fakePassword && code.value === "");
});
test("combined login never writes username or password into an OTP field", () => {
  const parent = form(); const user = input(parent, "email", { autocomplete: "username" });
  const password = input(parent, "password", { autocomplete: "current-password" });
  const code = input(parent, "text", { name: "mfa_token", autocomplete: "one-time-code", inputMode: "numeric", maxLength: 6 });
  let codeEvents = 0; code.addEventListener("input", () => { codeEvents++; }); code.addEventListener("change", () => { codeEvents++; });
  success(fill());
  assert(user.value === "fake-user" && password.value === fakePassword && code.value === "" && codeEvents === 0);
});
test("username-only credential stage outranks unrelated OTP UI", () => {
  const login = form(); const user = input(login, "email", { name: "account_email", autocomplete: "username" });
  const verification = form(); const code = input(verification, "text", { name: "verification_code", autocomplete: "one-time-code", inputMode: "numeric", maxLength: 6 });
  let codeEvents = 0; code.addEventListener("input", () => { codeEvents++; }); code.addEventListener("change", () => { codeEvents++; });
  success(inspect(), "username_only");
  success(fill(context(), expected, Date.now() + 1000, "username_only"), "username_only");
  assert(user.value === "fake-user" && code.value === "" && codeEvents === 0);
});
test("CAPTCHA, bot, device-approval and security controls are recognized without clicks", () => {
  for (const label of ["Complete CAPTCHA", "Start bot check", "Approve sign-in on your device", "Complete security challenge"]) {
    fixture.replaceChildren(); const button = document.createElement("button"); button.type = "button";
    button.textContent = label; fixture.append(button); let clicks = 0; button.addEventListener("click", () => { clicks++; });
    success(inspect(), "challenge", "website"); failure(fill(), "NO_LOGIN_FORM"); assert(clicks === 0);
  }
});
test("semantic CAPTCHA checkbox text is recognized without activation", () => {
  const checkbox = document.createElement("div"); checkbox.setAttribute("role", "checkbox");
  checkbox.textContent = "I'm not a robot"; checkbox.tabIndex = 0; fixture.append(checkbox);
  let clicks = 0; let keys = 0; checkbox.addEventListener("click", () => { clicks++; });
  checkbox.addEventListener("keydown", () => { keys++; });
  success(inspect(), "challenge", "website"); assert(clicks === 0 && keys === 0);
});
test("standard login fields remain fillable beside a website challenge", () => {
  const fields = pair(); const button = document.createElement("button"); button.type = "button";
  button.textContent = "Verify you are human"; fixture.append(button); let clicks = 0;
  button.addEventListener("click", () => { clicks++; }); success(fill());
  assert(fields.user.value === "fake-user" && fields.password.value === fakePassword && clicks === 0);
});
test("passkey-only UI is recognized without interaction", () => {
  const parent = form(); const button = document.createElement("button"); button.type = "button";
  button.textContent = "Sign in with a passkey"; parent.append(button);
  let clicks = 0; let submissions = 0; let keys = 0;
  button.addEventListener("click", () => { clicks++; });
  parent.addEventListener("submit", event => { event.preventDefault(); submissions++; });
  parent.addEventListener("keydown", () => { keys++; });
  success(inspect(), "passwordless");
  assert(clicks === 0 && submissions === 0 && keys === 0);
});
test("username webauthn autocomplete remains a username-only credential stage", () => {
  const user = input(form(), "text", { name: "username", autocomplete: "username webauthn" });
  success(inspect(), "username_only");
  success(fill(context(), expected, Date.now() + 1000, "username_only"), "username_only");
  assert(user.value === "fake-user");
});
test("username-only field outranks a visible passkey action without activating it", () => {
  const parent = form(); const user = input(parent, "email", { name: "account_email", autocomplete: "username" });
  const passkey = document.createElement("button"); passkey.type = "button"; passkey.textContent = "Sign in with a passkey"; parent.append(passkey);
  let clicks = 0; let submissions = 0; passkey.addEventListener("click", () => { clicks++; });
  parent.addEventListener("submit", event => { event.preventDefault(); submissions++; });
  success(inspect(), "username_only");
  success(fill(context(), expected, Date.now() + 1000, "username_only"), "username_only");
  assert(user.value === "fake-user" && clicks === 0 && submissions === 0);
});
test("username-only field outranks passive passkey explanatory text", () => {
  const parent = form(); const user = input(parent, "tel", { name: "mobile_login", autocomplete: "username" });
  const explanation = document.createElement("p"); explanation.textContent = "Passkeys let you sign in without a password."; parent.append(explanation);
  success(inspect(), "username_only");
  success(fill(context(), expected, Date.now() + 1000, "username_only"), "username_only");
  assert(user.value === "fake-user" && explanation.textContent === "Passkeys let you sign in without a password.");
});
test("password fallback works only after website action and a fresh KeyNest invocation", () => {
  const parent = form(); const button = document.createElement("button"); button.type = "button";
  button.textContent = "Use password instead"; button.setAttribute("aria-label", "Passkey options: use password instead");
  parent.append(button); let password = null; let clicks = 0;
  button.addEventListener("click", () => {
    clicks++; parent.replaceChildren(); password = input(parent, "password", { autocomplete: "current-password" });
  });
  success(inspect(), "passwordless"); assert(clicks === 0 && password === null);
  button.click(); assert(clicks === 1 && password !== null && password.value === "");
  success(inspect(), "password_only");
  success(fill(context(), expected, Date.now() + 1000, "password_only"), "password_only");
  assert(password.value === fakePassword && clicks === 1);
});
test("an available password form outranks a passkey alternative", () => {
  const parent = form(); const password = input(parent, "password", { autocomplete: "current-password" });
  const button = document.createElement("button"); button.type = "button"; button.textContent = "Use a security key";
  parent.append(button); let clicks = 0; button.addEventListener("click", () => { clicks++; });
  success(inspect(), "password_only");
  success(fill(context(), expected, Date.now() + 1000, "password_only"), "password_only");
  assert(password.value === fakePassword && clicks === 0);
});
test("combined login outranks a passkey alternative", () => {
  const parent = form(); const user = input(parent, "text", { name: "login", autocomplete: "username" });
  const password = input(parent, "password", { name: "password", autocomplete: "current-password" });
  const passkey = document.createElement("button"); passkey.type = "button"; passkey.textContent = "Use a security key"; parent.append(passkey);
  let clicks = 0; let submissions = 0; passkey.addEventListener("click", () => { clicks++; });
  parent.addEventListener("submit", event => { event.preventDefault(); submissions++; });
  success(inspect()); success(fill());
  assert(user.value === "fake-user" && password.value === fakePassword && clicks === 0 && submissions === 0);
});
test("hidden or non-actionable passkey text is not treated as a sign-in control", () => {
  const hidden = document.createElement("button"); hidden.textContent = "Use passkey"; hidden.hidden = true; fixture.append(hidden);
  const copy = document.createElement("p"); copy.textContent = "Learn about passkeys"; fixture.append(copy);
  success(inspect(), "unsupported");
});
test("developer diagnostics contain bounded structure and never input values", () => {
  const parent = form();
  const search = input(parent, "search", { name: "site-search", value: "FAKE-DIAGNOSTIC-SEARCH" });
  const contact = input(parent, "text", { id: "contact_email", autocomplete: "email", value: "FAKE-DIAGNOSTIC-CONTACT" });
  const weak = input(parent, "text", { name: "account", value: "FAKE-DIAGNOSTIC-ACCOUNT" });
  const hidden = input(parent, "password", { name: "password", value: "FAKE-DIAGNOSTIC-PASSWORD", hidden: true });
  const native = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value");
  for (const element of [search, contact, weak, hidden]) {
    Object.defineProperty(element, "value", { configurable: true, get() { throw new Error("diagnostics read value"); }, set(value) { native.set.call(this, value); } });
  }
  const result = diagnose();
  assert(result.ok === true && result.stage === "unsupported" && result.kind === "");
  assert(result.diagnostics.detectedStage === "unsupported" && result.diagnostics.origin === "https://fixture.test");
  assert(result.diagnostics.counts.inputs === 4 && result.diagnostics.fields.length === 4);
  assert(result.diagnostics.fields[1].id === "contact_email"
    && result.diagnostics.fields[2].score > 0
    && result.diagnostics.fields[2].rejectionReason === "username_candidate"
    && result.diagnostics.fields[3].visible === false);
  assert(result.diagnostics.fields.every(field => !Object.hasOwn(field, "value")
    && Number.isInteger(field.score) && typeof field.rejectionReason === "string"));
  const serialized = JSON.stringify(result);
  assert(!serialized.includes("FAKE-DIAGNOSTIC") && !serialized.includes("diagnostics read value"));
});
test("multiple plausible username-only fields are ambiguous", () => {
  const parent = form(); input(parent, "email"); input(parent, "email"); success(inspect(), "ambiguous");
});
test("nearby formless pair", () => {
  const fields = pair("text", fixture); success(fill()); assert(fields.password.value === fakePassword);
});
test("Facebook-like React wrappers accept username webauthn and leave unrelated fields untouched", () => {
  const unrelated = input(fixture, "search", { name: "site-search", value: "unchanged" });
  const loginPanel = document.createElement("div"); const userWrap = document.createElement("div");
  const passwordWrap = document.createElement("div"); fixture.append(loginPanel); loginPanel.append(userWrap, passwordWrap);
  const user = input(userWrap, "tel", { name: "email", autocomplete: "username webauthn" });
  const password = input(passwordWrap, "password", { name: "pass" });
  const seen = []; let submissions = 0;
  for (const name of ["input", "change"]) loginPanel.addEventListener(name, event => seen.push([event.target === user ? "user" : "password", event.type, event.bubbles]));
  loginPanel.addEventListener("submit", () => { submissions++; });
  success(fill());
  assert(user.value === "fake-user" && password.value === fakePassword && unrelated.value === "unchanged");
  assert(JSON.stringify(seen) === JSON.stringify([["user", "input", true], ["user", "change", true], ["password", "input", true], ["password", "change", true]]));
  assert(submissions === 0);
});
test("GitHub-style combined login fills semantic username and password fields only", () => {
  const parent = form();
  const user = input(parent, "text", { name: "login", autocomplete: "username" });
  const password = input(parent, "password", { name: "password", autocomplete: "current-password" });
  const submit = document.createElement("button"); submit.type = "submit"; submit.textContent = "Sign in"; parent.append(submit);
  let submissions = 0; let clicks = 0;
  parent.addEventListener("submit", event => { event.preventDefault(); submissions++; });
  submit.addEventListener("click", () => { clicks++; });
  success(fill());
  assert(user.value === "fake-user" && password.value === fakePassword);
  assert(submissions === 0 && clicks === 0);
});
test("Google-style username step fills only the semantic account field", () => {
  const parent = form();
  const user = input(parent, "email", { name: "account_identifier", autocomplete: "username" });
  const next = document.createElement("button"); next.type = "button"; next.textContent = "Next"; parent.append(next);
  let clicks = 0; let submissions = 0; next.addEventListener("click", () => { clicks++; });
  parent.addEventListener("submit", event => { event.preventDefault(); submissions++; });
  success(inspect(), "username_only");
  success(fill(context(), expected, Date.now() + 1000, "username_only"), "username_only");
  assert(user.value === "fake-user" && clicks === 0 && submissions === 0);
});
test("Google-style password step fills only the current password field", () => {
  const parent = form(); const account = document.createElement("p"); account.textContent = "Selected account"; parent.append(account);
  const password = input(parent, "password", { name: "account_password", autocomplete: "current-password" });
  success(inspect(), "password_only");
  success(fill(context(), expected, Date.now() + 1000, "password_only"), "password_only");
  assert(password.value === fakePassword && account.textContent === "Selected account");
});
test("Microsoft-style username step fills only the semantic email field", () => {
  const parent = form();
  const user = input(parent, "email", { name: "account_login", autocomplete: "username" });
  const alternative = document.createElement("a"); alternative.href = "#options"; alternative.textContent = "Sign-in options"; parent.append(alternative);
  let clicks = 0; alternative.addEventListener("click", event => { event.preventDefault(); clicks++; });
  success(inspect(), "username_only");
  success(fill(context(), expected, Date.now() + 1000, "username_only"), "username_only");
  assert(user.value === "fake-user" && clicks === 0);
});
test("Microsoft-style password step ignores the displayed account label", () => {
  const parent = form(); const account = document.createElement("span"); account.textContent = "Account already selected"; parent.append(account);
  const password = input(parent, "password", { name: "account_password", autocomplete: "current-password" });
  success(inspect(), "password_only");
  success(fill(context(), expected, Date.now() + 1000, "password_only"), "password_only");
  assert(password.value === fakePassword && account.textContent === "Account already selected");
});
test("Amazon-style staged login requires a fresh explicit fill for each DOM stage", () => {
  let parent = form();
  const user = input(parent, "email", { name: "account_email", autocomplete: "username" });
  let submissions = 0; parent.addEventListener("submit", event => { event.preventDefault(); submissions++; });
  success(inspect(), "username_only");
  success(fill(context(), expected, Date.now() + 1000, "username_only"), "username_only");
  assert(user.value === "fake-user" && submissions === 0);

  fixture.replaceChildren(); parent = form();
  const password = input(parent, "password", { name: "account_password", autocomplete: "current-password" });
  parent.addEventListener("submit", event => { event.preventDefault(); submissions++; });
  success(inspect(), "password_only");
  success(fill(context(), expected, Date.now() + 1000, "password_only"), "password_only");
  assert(password.value === fakePassword && submissions === 0);
});
test("generic SSO form fills a strongly associated combined login", () => {
  const parent = form();
  const tenant = input(parent, "text", { name: "organization", autocomplete: "organization", value: "unchanged" });
  const user = input(parent, "text", { name: "sso_username", autocomplete: "username" });
  const password = input(parent, "password", { name: "sso_password", autocomplete: "current-password" });
  success(fill());
  assert(tenant.value === "unchanged" && user.value === "fake-user" && password.value === fakePassword);
});
test("Discord-style login ignores a non-form QR alternative", () => {
  const layout = document.createElement("section"); const qrAlternative = document.createElement("aside");
  const login = document.createElement("form"); fixture.append(layout); layout.append(login, qrAlternative);
  const user = input(login, "text", { name: "login", autocomplete: "username" });
  const password = input(login, "password", { name: "password", autocomplete: "current-password" });
  success(fill()); assert(user.value === "fake-user" && password.value === fakePassword);
  assert(qrAlternative.children.length === 0);
});
test("LinkedIn-style modal wins over a hidden background login", () => {
  const background = document.createElement("div"); background.style.display = "none"; fixture.append(background);
  const hidden = pair("email", background);
  const modal = document.createElement("dialog"); modal.open = true; fixture.append(modal);
  const current = pair("text", modal); current.user.name = "session_key";
  current.password.name = "session_password"; current.password.autocomplete = "current-password";
  success(fill());
  assert(hidden.user.value === "" && hidden.password.value === "");
  assert(current.user.value === "fake-user" && current.password.value === fakePassword);
});
test("global search and negative-profile fields do not outrank the login pair", () => {
  const search = input(fixture, "text", { name: "global_search", value: "unchanged" });
  const login = form();
  for (const name of ["coupon", "shipping_address", "city", "firstName", "lastName", "message", "comment", "newsletter_email"]) {
    input(login, "text", { name });
  }
  const user = input(login, "text", { name: "account_identifier" });
  const password = input(login, "password", { name: "password" });
  success(fill());
  assert(search.value === "unchanged" && user.value === "fake-user" && password.value === fakePassword);
});
test("hidden responsive duplicate login is ignored", () => {
  const hiddenPanel = document.createElement("section"); hiddenPanel.style.visibility = "hidden"; fixture.append(hiddenPanel);
  const hidden = pair("email", hiddenPanel); const current = pair("email");
  success(fill());
  assert(hidden.user.value === "" && hidden.password.value === "");
  assert(current.user.value === "fake-user" && current.password.value === fakePassword);
});
test("visible signup new-password fields do not compete with a current-password login", () => {
  const login = form();
  const user = input(login, "text", { name: "username", autocomplete: "username" });
  const password = input(login, "password", { name: "password", autocomplete: "current-password" });
  const signup = form(); const signupEmail = input(signup, "email", { name: "signup_email", autocomplete: "email" });
  const signupPassword = input(signup, "password", { name: "new_password", autocomplete: "new-password" });
  const confirmation = input(signup, "password", { name: "confirm_password", autocomplete: "new-password" });
  success(fill());
  assert(user.value === "fake-user" && password.value === fakePassword);
  assert(signupEmail.value === "" && signupPassword.value === "" && confirmation.value === "");
});
test("a related current-password pair outranks an unrelated password field", () => {
  const unrelated = input(form(), "password", { name: "password" });
  const login = form(); const user = input(login, "email", { name: "email" });
  const password = input(login, "password", { name: "password", autocomplete: "current-password" });
  success(fill());
  assert(unrelated.value === "" && user.value === "fake-user" && password.value === fakePassword);
});
test("nearby React-like container associates fields without a shared form", () => {
  const panel = document.createElement("section"); fixture.append(panel);
  const user = input(panel, "text", { name: "login" }); const passwordForm = document.createElement("form");
  panel.append(passwordForm); const password = input(passwordForm, "password"); success(fill());
  assert(user.value === "fake-user" && password.value === fakePassword);
});
test("preflight reads no values and changes no fields", () => {
  const fields = pair();
  for (const element of [fields.user, fields.password]) Object.defineProperty(element, "value", { get() { throw new Error("unexpected read"); } });
  success(inspect());
});
test("readonly username ignored in favor of editable username", () => {
  const parent = form(); const readonly = input(parent, "email", { readOnly: true, value: "unchanged" });
  const user = input(parent, "text", { autocomplete: "username" }); const password = input(parent, "password");
  success(fill()); assert(readonly.value === "unchanged" && user.value === "fake-user" && password.value === fakePassword);
});
test("readonly-only username produces password-only stage", () => {
  const fields = pair(); fields.user.readOnly = true;
  success(inspect(), "password_only"); success(fill(context(), expected, Date.now() + 1000, "password_only"), "password_only");
  assert(fields.user.value === "" && fields.password.value === fakePassword);
});
test("hidden and disabled password fields ignored", () => {
  const parent = form(); const user = input(parent, "email");
  const hidden = input(parent, "password", { hidden: true }); const disabled = input(parent, "password", { disabled: true });
  const password = input(parent, "password"); success(fill());
  assert(user.value === "fake-user" && hidden.value === "" && disabled.value === "" && password.value === fakePassword);
});
test("readonly password and disabled fieldset leave a username-only stage", () => {
  const fields = pair(); fields.password.readOnly = true;
  success(inspect(), "username_only"); success(fill(context(), expected, Date.now() + 1000, "username_only"), "username_only");
  assert(fields.user.value === "fake-user" && fields.password.value === "");
  fields.password.readOnly = false; const fieldset = document.createElement("fieldset"); fieldset.disabled = true;
  fields.parent.append(fieldset); fieldset.append(fields.password);
  success(inspect(), "username_only"); success(fill(context(), expected, Date.now() + 1000, "username_only"), "username_only");
  assert(fields.password.value === "");
});
test("invisible ancestors, inert fields and zero rendered boxes are refused", () => {
  for (const style of ["display:none", "visibility:hidden", "opacity:0", "content-visibility:hidden"]) {
    fixture.replaceChildren(); const fields = pair(); fields.parent.style.cssText = style;
    failure(fill(), "NO_LOGIN_FORM"); assert(fields.password.value === "");
  }
  fixture.replaceChildren(); let fields = pair(); fields.parent.inert = true; failure(fill(), "NO_LOGIN_FORM");
  fixture.replaceChildren(); fields = pair(); fields.password.style.cssText = "width:0;height:0;border:0;padding:0";
  success(inspect(), "username_only"); success(fill(context(), expected, Date.now() + 1000, "username_only"), "username_only");
  assert(fields.user.value === "fake-user" && fields.password.value === "");
});
test("multiple login forms fail without writing any value", () => {
  const first = pair(); const second = pair(); failure(fill(), "AMBIGUOUS_LOGIN_FORM");
  assert(first.user.value === "" && first.password.value === "" && second.user.value === "" && second.password.value === "");
});
test("multiple password fields in one form fail", () => {
  const fields = pair(); input(fields.parent, "password"); failure(fill(), "AMBIGUOUS_LOGIN_FORM");
});
test("multiple password-only fields are ambiguous", () => {
  const parent = form(); input(parent, "password"); input(parent, "password"); success(inspect(), "ambiguous");
});
test("missing or new-password-only forms expose username-only stage", () => {
  let user = input(form(), "email"); success(inspect(), "username_only");
  success(fill(context(), expected, Date.now() + 1000, "username_only"), "username_only"); assert(user.value === "fake-user");
  fixture.replaceChildren(); const fields = pair(); fields.password.autocomplete = "new-password";
  success(inspect(), "username_only"); success(fill(context(), expected, Date.now() + 1000, "username_only"), "username_only");
  assert(fields.user.value === "fake-user" && fields.password.value === "");
});
test("visible signup password does not make one current-password login ambiguous", () => {
  const fields = pair(); const signup = input(fields.parent, "password", { autocomplete: "new-password" });
  success(fill()); assert(fields.password.value === fakePassword && signup.value === "");
});
test("autocomplete username wins over unrelated text/email fields", () => {
  const parent = form(); const email = input(parent, "email"); const user = input(parent, "text", { autocomplete: "username" });
  const other = input(parent, "text"); const password = input(parent, "password"); success(fill());
  assert(user.value === "fake-user" && email.value === "" && other.value === "" && password.value === fakePassword);
});
test("tied explicit username candidates fail closed", () => {
  const parent = form(); input(parent, "text", { autocomplete: "username" }); input(parent, "text", { autocomplete: "username" });
  input(parent, "password"); failure(fill(), "AMBIGUOUS_LOGIN_FORM");
});
test("fields outside the password form never selected", () => {
  const unrelated = input(form(), "email"); const fields = pair("text"); success(fill());
  assert(unrelated.value === "" && fields.user.value === "fake-user");
});
test("native setter bypasses instance setter and dispatches bubbling input/change", () => {
  const fields = pair(); const seen = []; let overriddenSetter = 0;
  for (const element of [fields.user, fields.password]) {
    const native = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value");
    Object.defineProperty(element, "value", { configurable: true, get() { return native.get.call(this); }, set() { overriddenSetter++; } });
  }
  for (const name of ["input", "change"]) fields.parent.addEventListener(name, event => seen.push([event.target === fields.user ? "user" : "password", event.type, event.bubbles]));
  success(fill()); assert(overriddenSetter === 0);
  assert(JSON.stringify(seen) === JSON.stringify([["user", "input", true], ["user", "change", true], ["password", "input", true], ["password", "change", true]]));
});
test("never submits, clicks, presses keys or writes credential markup/attributes", () => {
  const fields = pair(); let submissions = 0; let clicks = 0; let keys = 0;
  fields.parent.addEventListener("submit", event => { event.preventDefault(); submissions++; });
  fields.parent.submit = () => { submissions++; }; fields.parent.requestSubmit = () => { submissions++; };
  fields.parent.addEventListener("click", () => { clicks++; }); fields.parent.addEventListener("keydown", () => { keys++; });
  success(fill()); assert(submissions === 0 && clicks === 0 && keys === 0);
  assert(!fields.password.hasAttribute("value") && !fields.user.hasAttribute("value"));
  assert(!fixture.innerHTML.includes(fakePassword) && !fixture.innerHTML.includes("fake-user"));
});
test("username-only and password-only stages never submit", () => {
  for (const [stage, type, properties] of [["username_only", "email", {}], ["password_only", "password", {}]]) {
    fixture.replaceChildren(); const parent = form(); const field = input(parent, type, properties); let submissions = 0;
    parent.addEventListener("submit", event => { event.preventDefault(); submissions++; });
    parent.submit = () => { submissions++; }; parent.requestSubmit = () => { submissions++; };
    success(fill(context(), expected, Date.now() + 1000, stage), stage);
    assert(submissions === 0 && field.value === (stage === "username_only" ? "fake-user" : fakePassword));
  }
});
test("changed URL, unsupported URL, subframe and expired operation refuse before writes", () => {
  const fields = pair(); let window = context(); window.location.href = "https://evil.test/";
  failure(fill(window), "PAGE_CHANGED"); window = context(); window.location.protocol = "http:"; failure(fill(window), "PAGE_CHANGED");
  window = context(); window.top = {}; failure(fill(window), "PAGE_CHANGED"); failure(fill(context(), expected, Date.now() - 1), "PAGE_CHANGED");
  assert(fields.password.value === "" && fields.user.value === "");
});
test("DOM replacement during username event refuses password delivery", () => {
  const fields = pair(); fields.user.addEventListener("input", () => fields.password.remove());
  failure(fill(), "PAGE_CHANGED"); assert(fields.password.value === "");
});
test("navigation during username event refuses password delivery", () => {
  const fields = pair(); const window = context(); fields.user.addEventListener("input", () => { window.location.href = "https://evil.test"; });
  failure(fill(window), "PAGE_CHANGED"); assert(fields.password.value === "");
});
test("new ambiguity during username event refuses password delivery", () => {
  const fields = pair(); fields.user.addEventListener("input", () => input(fields.parent, "password"));
  failure(fill(), "PAGE_CHANGED"); assert(fields.password.value === "");
});
test("application clearing the password reports failure rather than success", () => {
  const fields = pair(); fields.password.addEventListener("change", () => { fields.password.value = ""; });
  failure(fill(), "NO_LOGIN_FORM");
});

test("username and password in one open shadow root fill", () => {
  const host = document.createElement("div"); fixture.append(host);
  const root = host.attachShadow({ mode: "open" }); const login = document.createElement("form"); root.append(login);
  const fields = pair("email", login); success(fill());
  assert(fields.user.value === "fake-user" && fields.password.value === fakePassword);
});

test("nested open shadow roots are traversed", () => {
  const outerHost = document.createElement("div"); fixture.append(outerHost);
  const outerRoot = outerHost.attachShadow({ mode: "open" }); const innerHost = document.createElement("div"); outerRoot.append(innerHost);
  const innerRoot = innerHost.attachShadow({ mode: "open" }); const fields = pair("text", innerRoot); fields.user.name = "username";
  success(fill()); assert(fields.user.value === "fake-user" && fields.password.value === fakePassword);
});

test("light-DOM username associates with password in an open shadow root", () => {
  const panel = document.createElement("section"); fixture.append(panel);
  const user = input(panel, "email", { name: "email" }); const host = document.createElement("div"); panel.append(host);
  const root = host.attachShadow({ mode: "open" }); const password = input(root, "password", { autocomplete: "current-password" });
  success(fill()); assert(user.value === "fake-user" && password.value === fakePassword);
});

test("closed shadow root remains unsupported", () => {
  const host = document.createElement("div"); fixture.append(host);
  const closed = host.attachShadow({ mode: "closed" }); pair("email", closed);
  success(inspect(), "unsupported");
});

test("unrelated open-shadow inputs are ignored", () => {
  const host = document.createElement("div"); fixture.append(host); const root = host.attachShadow({ mode: "open" });
  const newsletter = input(root, "email", { name: "newsletter_email" });
  const signup = input(root, "password", { name: "new_password", autocomplete: "new-password" });
  const fields = pair("email"); success(fill());
  assert(newsletter.value === "" && signup.value === "");
  assert(fields.user.value === "fake-user" && fields.password.value === fakePassword);
});

test("open-shadow traversal depth is bounded and fails closed", () => {
  let parent = fixture;
  for (let depth = 0; depth < 17; depth++) {
    const host = document.createElement("div"); parent.append(host);
    parent = host.attachShadow({ mode: "open" });
  }
  success(inspect(), "ambiguous");
});

test("SPA stage re-render is detected from the current DOM", () => {
  input(form(), "email"); success(inspect(), "username_only");
  fixture.replaceChildren(); input(form(), "password"); success(inspect(), "password_only");
});

test("separate inspections never reuse detached login inputs", () => {
  const stale = pair(); success(inspect()); fixture.replaceChildren();
  const current = pair(); success(fill());
  assert(stale.user.value === "" && stale.password.value === "");
  assert(current.user.value === "fake-user" && current.password.value === fakePassword);
});

await asyncTest("same-origin iframe login fills without submitting", async () => {
  const frame = document.createElement("iframe");
  const loaded = new Promise(resolve => frame.addEventListener("load", resolve, { once: true }));
  frame.srcdoc = '<!doctype html><form><input type="email" name="email"><input type="password" name="password"></form>';
  fixture.append(frame); await loaded;
  const topWindow = context();
  const frameWindow = {
    location: { protocol: "https:", href: "https://fixture.test/frame", origin: "https://fixture.test" },
    top: topWindow, parent: topWindow, frameElement: frame,
  };
  const childInvoke = frame.contentWindow.Function("window", "expected", "expires", "stage", "payload", `return (${fillLoginForm.toString()})(expected, expires, stage, payload);`);
  const fields = frame.contentDocument.querySelectorAll("input"); let submissions = 0;
  frame.contentDocument.querySelector("form").addEventListener("submit", event => { event.preventDefault(); submissions++; });
  success(childInvoke(frameWindow, expected, Date.now() + 1000, "combined", { username: "fake-user", password: fakePassword }));
  assert(fields[0].value === "fake-user" && fields[1].value === fakePassword && submissions === 0);
  fields[0].value = ""; fields[1].value = ""; frameWindow.location.origin = "https://other.test";
  failure(childInvoke(frameWindow, expected, Date.now() + 1000, "combined", { username: "fake-user", password: fakePassword }), "PAGE_CHANGED");
  assert(fields[0].value === "" && fields[1].value === "" && submissions === 0);
  frameWindow.location.origin = "https://fixture.test";
  fields[0].value = ""; fields[1].value = ""; frame.style.display = "none";
  failure(childInvoke(frameWindow, expected, Date.now() + 1000, "combined", { username: "fake-user", password: fakePassword }), "PAGE_CHANGED");
  assert(fields[0].value === "" && fields[1].value === "" && submissions === 0);
});

const OriginalMutationObserver = globalThis.MutationObserver;
let observerDisconnects = 0;
globalThis.MutationObserver = class extends OriginalMutationObserver {
  disconnect() { observerDisconnects++; return super.disconnect(); }
};
try {
  await asyncTest("form mounting shortly after inspection starts is detected", async () => {
    setTimeout(() => pair(), 25);
    success(await invoke(context(), expected, Date.now() + 500, null, null, 200));
    assert(observerDisconnects === 1);
  });
  await asyncTest("form mounting inside an existing open shadow root is detected", async () => {
    const host = document.createElement("div"); fixture.append(host); const root = host.attachShadow({ mode: "open" });
    setTimeout(() => pair("email", root), 25);
    success(await invoke(context(), expected, Date.now() + 500, null, null, 200));
    assert(observerDisconnects === 2);
  });
  await asyncTest("delayed inspection timeout fails safely and disconnects", async () => {
    success(await invoke(context(), expected, Date.now() + 500, null, null, 40), "unsupported");
    assert(observerDisconnects === 3);
  });
  await asyncTest("Instagram-style SPA login mounts late and is filled only on a fresh explicit action", async () => {
    let user = null; let password = null; let submissions = 0;
    setTimeout(() => {
      const shell = document.createElement("section"); fixture.append(shell);
      const parent = document.createElement("form"); shell.append(parent);
      user = input(parent, "text", { name: "login_identifier", autocomplete: "username" });
      password = input(parent, "password", { name: "login_password", autocomplete: "current-password" });
      parent.addEventListener("submit", event => { event.preventDefault(); submissions++; });
    }, 25);
    success(await invoke(context(), expected, Date.now() + 500, null, null, 200));
    assert(user.value === "" && password.value === "" && submissions === 0);
    success(fill());
    assert(user.value === "fake-user" && password.value === fakePassword && submissions === 0);
    assert(observerDisconnects === 4);
  });
} finally {
  globalThis.MutationObserver = OriginalMutationObserver;
}

const failed = results.filter(result => !result.ok);
document.documentElement.dataset.result = failed.length ? "failed" : "passed";
document.documentElement.dataset.count = String(results.length);
document.documentElement.dataset.failures = failed.map(result => result.name).join("; ");
document.getElementById("results").textContent = JSON.stringify(results);
