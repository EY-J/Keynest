// Self-contained function serialized by executeScript into the ISOLATED world.
// No imports, global state, page/native messaging or retained observers/timers.
export function fillLoginForm(expectedUrl, expiresAt, expectedStage = null, payload = null, waitMs = 0, diagnosticMode = false) {
  const fail = code => ({ ok: false, code });
  const success = (stage, kind = null) => kind === null ? { ok: true, stage } : { ok: true, stage, kind };
  function composedParent(node) {
    if (node.parentElement) return node.parentElement;
    const root = typeof node.getRootNode === "function" ? node.getRootNode() : null;
    return root?.nodeType === 11 && root.mode === "open" ? root.host : null;
  }
  function frameElementIsVisible(frame) {
    if (!frame?.isConnected) return false;
    const bounds = frame.getBoundingClientRect();
    if (bounds.width <= 0 || bounds.height <= 0
      || !Array.from(frame.getClientRects()).some(rect => rect.width > 0 && rect.height > 0)) return false;
    for (let node = frame; node; node = composedParent(node)) {
      if (node.hidden || node.inert) return false;
      const view = node.ownerDocument?.defaultView;
      const style = view?.getComputedStyle(node);
      if (!style || style.display === "none" || style.visibility === "hidden"
        || style.visibility === "collapse" || style.opacity === "0"
        || style.contentVisibility === "hidden") return false;
    }
    return true;
  }
  function destinationIsCurrent() {
    try {
      const topLocation = window.top.location;
      if (topLocation.protocol !== "https:" || topLocation.href !== expectedUrl
        || window.location.protocol !== "https:" || window.location.origin !== topLocation.origin
        || document.visibilityState !== "visible" || Date.now() > expiresAt) return false;
      let currentWindow = window;
      while (currentWindow !== currentWindow.top) {
        if (!frameElementIsVisible(currentWindow.frameElement)) return false;
        currentWindow = currentWindow.parent;
      }
      return true;
    } catch {
      return false;
    }
  }
  try {
    if (!destinationIsCurrent()) return fail("PAGE_CHANGED");
    function collectInputs() {
      const inputs = [];
      const actions = [];
      const challengeIndicators = [];
      const roots = [];
      let elementCount = 0;
      let rootCount = 0;
      let ambiguous = false;
      function visit(root, depth) {
        if (ambiguous || depth > 16 || ++rootCount > 100) { ambiguous = true; return; }
        roots.push(root);
        const walker = document.createTreeWalker(root, 1);
        for (let node = walker.nextNode(); node; node = walker.nextNode()) {
          if (++elementCount > 10000) { ambiguous = true; return; }
          if (node instanceof HTMLInputElement && inputs.push(node) > 2000) {
            ambiguous = true;
            return;
          }
          if (typeof node.matches === "function"
            && node.matches("button,a[href],[role='button'],input[type='button'],input[type='submit']")
            && actions.push(node) > 2000) {
            ambiguous = true;
            return;
          }
          if (typeof node.matches === "function"
            && node.matches("iframe,h1,h2,h3,[data-sitekey],[role='checkbox'],[role='alert'],[role='status'],[id*='captcha' i],[class*='captcha' i],[id*='challenge' i],[class*='challenge' i],[id*='device-approval' i],[class*='device-approval' i]")
            && challengeIndicators.push(node) > 2000) {
            ambiguous = true;
            return;
          }
          const shadow = node.shadowRoot;
          if (shadow?.mode === "open") visit(shadow, depth + 1);
          if (ambiguous) return;
        }
      }
      visit(document, 0);
      return { inputs, actions, challengeIndicators, roots, ambiguous };
    }
    const collected = collectInputs();
    if (collected.ambiguous) return success("ambiguous");
    const inputs = collected.inputs;
    function visible(element) {
      if (!(element instanceof Element) || !element.isConnected || element.ownerDocument !== document) return false;
      const bounds = element.getBoundingClientRect();
      if (bounds.width <= 0 || bounds.height <= 0
        || !Array.from(element.getClientRects()).some(rect => rect.width > 0 && rect.height > 0)) return false;
      for (let node = element; node; node = composedParent(node)) {
        if (node.hidden || node.inert) return false;
        const style = getComputedStyle(node);
        if (style.display === "none" || style.visibility === "hidden" || style.visibility === "collapse"
          || style.opacity === "0" || style.contentVisibility === "hidden") return false;
      }
      return true;
    }
    function usable(input) {
      return input instanceof HTMLInputElement && visible(input)
        && !input.matches(":disabled") && !input.readOnly;
    }
    const autocompleteTokens = input => input.autocomplete.toLowerCase().trim().split(/\s+/).filter(Boolean);
    const hasAutocomplete = (input, token) => autocompleteTokens(input).includes(token);
    const identity = input => `${input.name} ${input.id}`.toLowerCase();
    const usernameTypes = new Set(["email", "text", "tel"]);
    const challengeTypes = new Set(["text", "tel", "number", "password"]);
    const loginIdentity = input => /(email|user|username|login)/.test(identity(input));
    const phoneIdentity = input => /(phone|mobile)/.test(identity(input));
    const challengeIdentity = input => /(otp|code|verification|token|mfa|2fa)/.test(identity(input));
    const codeLength = input => {
      const value = input.getAttribute("maxlength");
      return value !== null && /^\d+$/.test(value) ? Number(value) : input.maxLength;
    };
    const numericHint = input => input.type === "number"
      || ["numeric", "decimal"].includes(input.inputMode.toLowerCase())
      || /\\d|\[0-9\]/.test(input.getAttribute("pattern") || "");
    const oneCharacterInput = input => codeLength(input) === 1 && challengeTypes.has(input.type);
    const shortNumericChallenge = input => {
      const length = codeLength(input);
      return length >= 4 && length <= 8 && numericHint(input) && challengeTypes.has(input.type);
    };
    const explicitChallenge = input => usable(input) && (hasAutocomplete(input, "one-time-code")
      || challengeIdentity(input) || shortNumericChallenge(input));
    function nonLoginIdentity(input) {
      const value = identity(input);
      return /(search|contact|newsletter|coupon|promo|city|first(?:_|-|\s)?name|last(?:_|-|\s)?name|firstname|lastname|message|comment)/.test(value)
        || (/address/.test(value) && !/email/.test(value));
    }
    function usernameMetadata(input) {
      if (!usable(input) || !usernameTypes.has(input.type) || nonLoginIdentity(input)) return null;
      const tokens = autocompleteTokens(input);
      const explicitUsername = tokens.includes("username");
      const explicitEmail = tokens.includes("email");
      const explicitPhone = tokens.some(token => ["tel", "tel-national", "tel-local"].includes(token));
      if (tokens.includes("new-password") || tokens.includes("current-password")
        || explicitChallenge(input) || oneCharacterInput(input)) return null;
      if (tokens.some(token => ["name", "given-name", "family-name", "street-address",
        "address-line1", "address-line2", "address-line3", "address-level1", "address-level2",
        "address-level3", "address-level4", "postal-code", "country", "country-name",
        "organization", "organization-title", "shipping", "billing"].includes(token))) return null;
      if (!explicitUsername && !explicitEmail && !explicitPhone
        && tokens.some(token => !["on", "off", "webauthn"].includes(token)
          && !token.startsWith("section-"))) return null;
      const namedLikeLogin = loginIdentity(input);
      const namedLikePhone = phoneIdentity(input);
      const tier = explicitUsername || explicitEmail || input.type === "email" || namedLikeLogin
        ? 3 : explicitPhone || input.type === "tel" || namedLikePhone ? 2 : 1;
      return {
        explicit: explicitUsername || explicitEmail || explicitPhone,
        tier,
        semantic: explicitUsername ? 700 : explicitEmail ? 650 : input.type === "email" ? 600
          : namedLikeLogin ? 550 : explicitPhone ? 500 : input.type === "tel" ? 450
            : namedLikePhone ? 400 : 100,
      };
    }
    function commonContainerDistance(left, right) {
      const leftAncestors = new Map();
      let node = composedParent(left);
      for (let depth = 1; node && depth <= 8; depth++, node = composedParent(node)) leftAncestors.set(node, depth);
      node = composedParent(right);
      for (let depth = 1; node && depth <= 8; depth++, node = composedParent(node)) {
        const leftDepth = leftAncestors.get(node);
        if (leftDepth && node !== document.body && node !== document.documentElement) return leftDepth + depth;
      }
      return 0;
    }
    function groupedChallenge(currentInputs) {
      const boxes = currentInputs.filter(input => usable(input) && oneCharacterInput(input));
      if (boxes.length < 4) return false;
      return boxes.some(anchor => boxes.filter(candidate => {
        if (anchor.form || candidate.form) return anchor.form !== null && anchor.form === candidate.form;
        return anchor === candidate || commonContainerDistance(anchor, candidate) > 0;
      }).length >= 4);
    }
    const challengeDetected = currentInputs => currentInputs.some(explicitChallenge)
      || groupedChallenge(currentInputs);
    function association(input, password) {
      if (password.form && input.form) {
        if (input.form === password.form) return { rank: 4, distance: 0 };
        if (input.getRootNode() === password.getRootNode()) return { rank: 0, distance: 0 };
      }
      const distance = commonContainerDistance(input, password);
      return distance ? { rank: 3, distance } : { rank: 0, distance: 0 };
    }
    function usernameCandidates(currentInputs, password = null, strongOnly = false) {
      const passwordIndex = password ? currentInputs.indexOf(password) : -1;
      return currentInputs.flatMap((input, index) => {
        const metadata = usernameMetadata(input);
        if (!metadata || (strongOnly && metadata.tier < 2) || (password && index >= passwordIndex)) return [];
        const relation = password ? association(input, password) : { rank: 0, distance: 0 };
        const domDistance = passwordIndex - index;
        if (password && (!relation.rank || (metadata.tier === 1 && domDistance > 12))) return [];
        const proximity = password
          ? Math.max(0, 200 - domDistance) + (relation.distance ? Math.max(0, 60 - relation.distance * 5) : 0)
          : 0;
        return [{
          input,
          score: relation.rank * 100000 + metadata.semantic * 100 + proximity,
          semantic: metadata.semantic,
          tier: metadata.tier,
          relationRank: relation.rank,
          explicit: metadata.explicit,
        }];
      }).sort((left, right) => right.score - left.score);
    }
    function selectUsername(candidates) {
      if (!candidates.length) return null;
      if (candidates.length > 1 && (candidates[0].score === candidates[1].score
        || (candidates[0].explicit && candidates[1].explicit
          && candidates[0].relationRank === candidates[1].relationRank
          && candidates[0].semantic === candidates[1].semantic))) return { ambiguous: true };
      return candidates[0];
    }
    function passwordMetadata(input) {
      if (!usable(input) || input.type !== "password" || hasAutocomplete(input, "new-password")
        || explicitChallenge(input) || oneCharacterInput(input)) return null;
      const current = hasAutocomplete(input, "current-password");
      return {
        current,
        semantic: current ? 600 : /(pass|password|login)/.test(identity(input)) ? 450 : 300,
      };
    }
    function passwordCandidates(currentInputs) {
      return currentInputs.flatMap(password => {
        const metadata = passwordMetadata(password);
        if (!metadata) return [];
        const usernames = usernameCandidates(currentInputs, password);
        const username = selectUsername(usernames);
        const rankedUsername = username?.ambiguous ? usernames[0] : username;
        const relationRank = rankedUsername?.relationRank ?? 0;
        const usernameSemantic = rankedUsername?.semantic ?? 0;
        return [{
          password,
          username,
          score: relationRank * 1000000 + metadata.semantic * 1000 + usernameSemantic,
          signature: `${relationRank}:${metadata.semantic}:${rankedUsername?.tier ?? 0}:${usernameSemantic}:${rankedUsername?.explicit ? 1 : 0}`,
        }];
      }).sort((left, right) => right.score - left.score);
    }
    function selectPassword(candidates) {
      if (!candidates.length) return null;
      if (candidates.length > 1 && (candidates[0].score === candidates[1].score
        || candidates[0].signature === candidates[1].signature)) return { ambiguous: true };
      return candidates[0];
    }
    function passwordlessDetected(currentInputs, currentActions) {
      if (currentInputs.some(input => usable(input) && hasAutocomplete(input, "webauthn"))) return true;
      const signal = /\b(?:passkeys?|passwordless|webauthn|fido2?|security[\s_-]+keys?|magic[\s_-]+links?)\b/i;
      return currentActions.some(action => {
        if (!visible(action) || action.matches(":disabled")
          || action.getAttribute("aria-disabled")?.toLowerCase() === "true") return false;
        const description = [action.textContent, action.getAttribute("aria-label"),
          action.getAttribute("title"), action.getAttribute("name"), action.getAttribute("id"),
          action.getAttribute("value")].filter(value => typeof value === "string").join(" ");
        return description.length <= 2000 && signal.test(description);
      });
    }
    function websiteChallengeDetected(currentActions, currentIndicators) {
      const actionable = new Set(currentActions);
      const candidates = new Set([...currentActions, ...currentIndicators]);
      const signal = /\b(?:captcha|bot[\s_-]+check|security[\s_-]+(?:challenge|check|verification)|device[\s_-]+approval|verification[\s_-]+(?:step|required))\b|\b(?:verify|confirm|prove)\s+(?:that\s+)?you(?:'re|\s+are)?\s+(?:a\s+)?human\b|\b(?:i\s+am|i'm)\s+not\s+a\s+robot\b|\b(?:approve|confirm)\s+(?:the\s+)?(?:sign[\s-]*in|login|request)(?:\s+request)?\s+(?:on|from|with)\s+(?:your\s+)?(?:phone|device)\b|\bcheck\s+(?:your\s+)?(?:phone|device)\b/i;
      return Array.from(candidates).some(element => {
        if (!visible(element)) return false;
        if (element.hasAttribute("data-sitekey")) return true;
        const mayUseText = actionable.has(element)
          || element.matches("h1,h2,h3,[role='checkbox'],[role='alert'],[role='status']");
        const description = [mayUseText ? element.textContent : "",
          element.getAttribute("aria-label"), element.getAttribute("title"),
          element.getAttribute("name"), element.getAttribute("id"), element.getAttribute("class")]
          .filter(value => typeof value === "string").join(" ");
        return description.length <= 2000 && signal.test(description);
      });
    }
    function detect(currentInputs, currentActions, currentIndicators) {
      const strongUsernames = usernameCandidates(currentInputs, null, true);
      const password = selectPassword(passwordCandidates(currentInputs));
      if (password) {
        if (password.ambiguous || password.username?.ambiguous) return { stage: "ambiguous" };
        if (password.username) {
          return { stage: "combined", username: password.username.input, password: password.password };
        }
        if (strongUsernames.length) return { stage: "ambiguous" };
        return { stage: "password_only", password: password.password };
      }
      const username = selectUsername(strongUsernames);
      if (username?.ambiguous) return { stage: "ambiguous" };
      if (username) return { stage: "username_only", username: username.input };
      if (challengeDetected(currentInputs)) return { stage: "challenge", kind: "otp" };
      if (websiteChallengeDetected(currentActions, currentIndicators)) {
        return { stage: "challenge", kind: "website" };
      }
      if (passwordlessDetected(currentInputs, currentActions)) return { stage: "passwordless" };
      return { stage: "unsupported" };
    }
    function diagnosticsFor(detectedStage, currentInputs, currentActions, currentIndicators) {
      const usernameRanks = usernameCandidates(currentInputs);
      const passwordRanks = passwordCandidates(currentInputs);
      const usernameScores = new Map(usernameRanks.map(candidate => [candidate.input, candidate.score]));
      const passwordScores = new Map(passwordRanks.map(candidate => [candidate.password, candidate.score]));
      const grouped = groupedChallenge(currentInputs);
      const otpInputs = currentInputs.filter(input => explicitChallenge(input)
        || (grouped && usable(input) && oneCharacterInput(input)));
      const otpInputSet = new Set(otpInputs);
      const safeAttribute = (input, name) => (input.getAttribute(name) || "")
        .replace(/[\u0000-\u001f\u007f-\u009f]/g, "").slice(0, 128);
      function rejectionReason(input) {
        if (!visible(input)) return "not_visible";
        if (input.matches(":disabled")) return "disabled";
        if (input.readOnly) return "readonly";
        if (otpInputSet.has(input) || oneCharacterInput(input)) return "verification_code";
        if (hasAutocomplete(input, "new-password")) return "new_password";
        if (passwordScores.has(input)) return "password_candidate";
        if (usernameScores.has(input)) return "username_candidate";
        if (!usernameTypes.has(input.type) && input.type !== "password") return "unsupported_type";
        return "non_login_metadata";
      }
      const fields = currentInputs.slice(0, 100).map((input, index) => ({
        index,
        type: safeAttribute(input, "type") || input.type.slice(0, 32),
        autocomplete: safeAttribute(input, "autocomplete"),
        id: safeAttribute(input, "id"),
        name: safeAttribute(input, "name"),
        visible: visible(input),
        score: usernameScores.get(input) ?? passwordScores.get(input) ?? 0,
        rejectionReason: rejectionReason(input),
      }));
      return {
        origin: window.location.origin,
        counts: {
          inputs: currentInputs.length,
          visibleInputs: currentInputs.filter(visible).length,
          usernameCandidates: usernameRanks.length,
          passwordCandidates: passwordRanks.length,
          otpCandidates: otpInputs.length,
          interactiveControls: currentActions.length,
          challengeIndicators: currentIndicators.length,
        },
        fields,
        truncated: currentInputs.length > fields.length,
        detectedStage,
      };
    }
    const detected = detect(inputs, collected.actions, collected.challengeIndicators);
    if (payload === null) {
      if (diagnosticMode === true) {
        return {
          ok: true,
          stage: detected.stage,
          kind: detected.kind ?? "",
          diagnostics: diagnosticsFor(detected.stage, inputs, collected.actions, collected.challengeIndicators),
        };
      }
      // Preflight never reads input values. A short observer is created only by
      // an explicit popup action and is always disconnected on success/timeout.
      const boundedWaitMs = Number.isFinite(waitMs)
        ? Math.min(2500, Math.max(0, Math.floor(waitMs))) : 0;
      if (detected.stage !== "unsupported" || boundedWaitMs === 0
        || typeof MutationObserver !== "function" || !document.documentElement) {
        return success(detected.stage, detected.kind ?? null);
      }
      return new Promise(resolve => {
        let observer = null;
        let timer = null;
        let settled = false;
        const observedRoots = new Set();
        function finish(result) {
          if (settled) return;
          settled = true;
          observer?.disconnect();
          if (timer !== null) clearTimeout(timer);
          resolve(result);
        }
        function inspectFresh() {
          try {
            if (!destinationIsCurrent()) { finish(fail("PAGE_CHANGED")); return; }
            const currentCollection = collectInputs();
            if (currentCollection.ambiguous) { finish(success("ambiguous")); return; }
            observeRoots(currentCollection.roots);
            const current = detect(currentCollection.inputs, currentCollection.actions,
              currentCollection.challengeIndicators);
            if (current.stage !== "unsupported") finish(success(current.stage, current.kind ?? null));
          } catch {
            finish(fail("NO_LOGIN_FORM"));
          }
        }
        function observeRoots(roots) {
          for (const root of roots) {
            if (observedRoots.has(root)) continue;
            observer.observe(root, {
              childList: true,
              subtree: true,
              attributes: true,
              attributeFilter: ["type", "name", "id", "autocomplete", "disabled", "readonly", "hidden", "inert", "class", "style", "aria-label", "aria-disabled", "title", "role", "data-sitekey"],
            });
            observedRoots.add(root);
          }
        }
        observer = new MutationObserver(inspectFresh);
        observeRoots(collected.roots);
        timer = setTimeout(() => {
          finish(destinationIsCurrent() ? success("unsupported") : fail("PAGE_CHANGED"));
        }, boundedWaitMs);
        inspectFresh(); // close the observe/setup race with a new DOM query
      });
    }
    if (!new Set(["combined", "username_only", "password_only"]).has(expectedStage)) return fail("NO_LOGIN_FORM");
    if (detected.stage !== expectedStage) {
      if (detected.stage === "ambiguous") return fail("AMBIGUOUS_LOGIN_FORM");
      if (detected.stage === "unsupported" || detected.stage === "challenge"
        || detected.stage === "passwordless") return fail("NO_LOGIN_FORM");
      return fail("PAGE_CHANGED");
    }
    const needsUsername = expectedStage !== "password_only";
    const needsPassword = expectedStage !== "username_only";
    if ((needsUsername && (typeof payload.username !== "string" || !payload.username))
      || (needsPassword && (typeof payload.password !== "string" || !payload.password))) return fail("NO_LOGIN_FORM");
    const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set;
    if (!setter) return fail("NO_LOGIN_FORM");
    const stillUsable = () => {
      if (!destinationIsCurrent()) return false;
      const currentCollection = collectInputs();
      if (currentCollection.ambiguous) return false;
      const current = detect(currentCollection.inputs, currentCollection.actions,
        currentCollection.challengeIndicators);
      return current.stage === expectedStage && current.username === detected.username
        && current.password === detected.password;
    };
    function setInput(input, value) {
      if (!stillUsable()) return false;
      setter.call(input, value);
      input.dispatchEvent(new Event("input", { bubbles: true }));
      input.dispatchEvent(new Event("change", { bubbles: true }));
      return stillUsable();
    }
    if (needsUsername && !setInput(detected.username, payload.username)) return fail("PAGE_CHANGED");
    if (needsPassword && !setInput(detected.password, payload.password)) return fail("PAGE_CHANGED");
    if ((needsUsername && detected.username.value !== payload.username)
      || (needsPassword && detected.password.value !== payload.password)) return fail("NO_LOGIN_FORM");
    return success(expectedStage); // Never return input values or page content.
  } catch {
    return fail("NO_LOGIN_FORM");
  } finally {
    if (payload) { payload.username = ""; payload.password = ""; }
    payload = null;
  }
}
