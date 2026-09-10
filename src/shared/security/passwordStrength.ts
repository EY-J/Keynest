export interface StrengthResult {
  score: 0 | 1 | 2 | 3 | 4;
  label: string;
  patterns: string[];
  suggestions: string[];
}

const COMMON_PASSWORDS = [
  "password",
  "passw0rd",
  "letmein",
  "qwerty",
  "admin",
  "welcome",
  "iloveyou",
  "abc123",
] as const;

function hasSequentialPattern(characters: string[]) {
  return characters.some((_, index) => {
    const sequence = characters.slice(index, index + 4);
    if (sequence.length < 4) return false;
    const isAsciiLetter = sequence.every((value) => /^[a-z]$/.test(value));
    const isDigit = sequence.every((value) => /^\d$/.test(value));
    if (!isAsciiLetter && !isDigit) return false;
    const codes = sequence.map((value) => value.codePointAt(0) ?? 0);
    return codes.every((value, codeIndex) => {
      if (codeIndex === 0) return true;
      return value - codes[codeIndex - 1] === codes[1] - codes[0];
    }) && Math.abs(codes[1] - codes[0]) === 1;
  });
}

function hasRepeatedRun(characters: string[]) {
  return characters.some(
    (character, index) =>
      index >= 2 &&
      character === characters[index - 1] &&
      character === characters[index - 2],
  );
}

function hasRepeatedChunk(characters: string[]) {
  for (let chunkLength = 2; chunkLength <= Math.min(6, characters.length / 2); chunkLength += 1) {
    for (let start = 0; start + chunkLength * 2 <= characters.length; start += 1) {
      const first = characters.slice(start, start + chunkLength).join("");
      const second = characters.slice(start + chunkLength, start + chunkLength * 2).join("");
      if (first === second) return true;
    }
  }
  return false;
}

export function analyzePassword(password: string): StrengthResult {
  if (!password) {
    return { score: 0, label: "Very Weak", patterns: [], suggestions: [] };
  }

  const hasLower = /[a-z]/.test(password);
  const hasUpper = /[A-Z]/.test(password);
  const hasDigit = /\d/.test(password);
  const hasSymbol = /[^a-zA-Z0-9]/.test(password);
  const lowerPassword = password.toLowerCase();
  const characters = Array.from(password);
  const lowerCharacters = Array.from(lowerPassword);
  const characterCount = characters.length;
  const uniqueCharacterCount = new Set(characters).size;
  const patterns: string[] = [];
  let penalty = 0;
  let poolSize = 0;

  if (hasLower) poolSize += 26;
  if (hasUpper) poolSize += 26;
  if (hasDigit) poolSize += 10;
  if (hasSymbol) poolSize += 32;

  const commonMatch = COMMON_PASSWORDS.find((candidate) =>
    lowerPassword.includes(candidate),
  );
  if (commonMatch) {
    patterns.push(`Common password pattern "${commonMatch}"`);
    penalty += 30;
  }
  const sequentialPattern = hasSequentialPattern(lowerCharacters);
  if (sequentialPattern) {
    patterns.push("Sequential characters");
    penalty += 20;
  }
  const keyboardPattern = /qwert|asdf|zxcv/.test(lowerPassword);
  if (keyboardPattern) {
    patterns.push("Keyboard pattern");
    penalty += 30;
  }
  const repeatedRun = hasRepeatedRun(characters);
  if (repeatedRun) {
    patterns.push("Repeated characters");
    penalty += 24;
  }
  const repeatedChunk = hasRepeatedChunk(characters);
  if (repeatedChunk) {
    patterns.push("Repeated character pattern");
    penalty += 24;
  }
  const datePattern = /(19|20)\d{2}|\b\d{1,2}[/-]\d{1,2}(?:[/-]\d{2,4})?\b/.test(password);
  if (datePattern) {
    patterns.push("Date-like pattern");
    penalty += 15;
  }
  const hasLowDiversity =
    characterCount >= 8 && uniqueCharacterCount / characterCount < 0.35;
  if (hasLowDiversity) {
    patterns.push("Too few unique characters");
    penalty += 30;
  }

  const estimatedEntropy = characterCount * Math.log2(Math.max(poolSize, 1));
  const effectiveEntropy = Math.max(0, estimatedEntropy - penalty);

  let score: StrengthResult["score"];
  let label: string;
  if (effectiveEntropy < 25) {
    score = 0;
    label = "Very Weak";
  } else if (effectiveEntropy < 40) {
    score = 1;
    label = "Weak";
  } else if (effectiveEntropy < 55) {
    score = 2;
    label = "Fair";
  } else if (effectiveEntropy < 70) {
    score = 3;
    label = "Strong";
  } else {
    score = 4;
    label = "Very Strong";
  }

  const hasCriticalPattern =
    hasLowDiversity ||
    repeatedChunk ||
    (Boolean(commonMatch) && characterCount <= 16) ||
    (keyboardPattern && characterCount <= 16);
  if (hasCriticalPattern && score > 1) {
    score = 1;
    label = "Weak";
  }

  const suggestions: string[] = [];
  if (characterCount < 12) suggestions.push("Use at least 12 characters");
  if (!hasUpper) suggestions.push("Add uppercase letters");
  if (!hasDigit) suggestions.push("Add numbers");
  if (!hasSymbol) suggestions.push("Add symbols");
  if (patterns.length > 0) suggestions.push("Avoid predictable patterns");

  return { score, label, patterns, suggestions };
}
