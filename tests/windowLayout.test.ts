import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import tauriConfig from "../src-tauri/tauri.conf.json";

const appStyles = readFileSync(resolve(process.cwd(), "src/App.css"), "utf8");

type MediaRuleLike = CSSRule & {
  conditionText: string;
  cssRules: CSSRuleList;
};

function isMediaRule(rule: CSSRule): rule is MediaRuleLike {
  return "conditionText" in rule && "cssRules" in rule;
}

function isStyleRule(rule: CSSRule): rule is CSSStyleRule {
  return "selectorText" in rule && "style" in rule;
}

function compactWindowRules(): CSSStyleRule[] {
  const style = document.createElement("style");
  style.textContent = appStyles;
  document.head.append(style);

  const mediaRule = Array.from(style.sheet?.cssRules ?? []).find(
    (rule): rule is MediaRuleLike =>
      isMediaRule(rule) &&
      rule.conditionText === "(max-width: 1100px), (max-height: 760px)",
  );

  return Array.from(mediaRule?.cssRules ?? []).filter(isStyleRule);
}

function ruleFor(rules: CSSStyleRule[], selector: string) {
  const rule = rules.find((candidate) => candidate.selectorText === selector);
  expect(rule, `Missing compact layout rule for ${selector}`).toBeDefined();
  return rule!;
}

describe("the compact default window", () => {
  it("cannot be resized below its 1000 by 700 default dimensions", () => {
    expect(tauriConfig.app.windows[0]).toMatchObject({
      width: 1000,
      height: 700,
      minWidth: 1000,
      minHeight: 700,
    });
  });

  it("reduces Home spacing and typography within the compact window envelope", () => {
    const rules = compactWindowRules();

    expect(ruleFor(rules, ".topbar").style.minHeight).toBe("76px");
    expect(ruleFor(rules, ".hero").style.padding).toBe("60px 0px 58px");
    expect(ruleFor(rules, ".hero h1").style.fontSize).toBe(
      "clamp(2.8rem, 5vw, 3.1rem)",
    );
    expect(ruleFor(rules, ".feature-card").style.minHeight).toBe("270px");
  });
});
