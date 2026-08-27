const PASSWORD_LENGTH = 24;
const CHARACTERS_PER_GROUP = 2;

const CHARACTER_GROUPS = [
  "ABCDEFGHJKLMNPQRSTUVWXYZ",
  "abcdefghijkmnopqrstuvwxyz",
  "23456789",
  "!@#$%^&*()-_=+[]{}:,.?",
] as const;

function cryptoRandomBelow(max: number): number {
  const sample = new Uint32Array(1);
  const unbiasedLimit = Math.floor(0x1_0000_0000 / max) * max;
  let value: number;

  do {
    globalThis.crypto.getRandomValues(sample);
    value = sample[0];
  } while (value >= unbiasedLimit);

  return value % max;
}

function randomCharacter(pool: string): string {
  return pool[cryptoRandomBelow(pool.length)];
}

function secureShuffle(characters: string[]): string {
  for (let index = characters.length - 1; index > 0; index -= 1) {
    const swapIndex = cryptoRandomBelow(index + 1);
    [characters[index], characters[swapIndex]] = [characters[swapIndex], characters[index]];
  }
  return characters.join("");
}

export function generateAdvancedPassword(): string {
  const passwordCharacters = CHARACTER_GROUPS.flatMap((group) =>
    Array.from(
      { length: CHARACTERS_PER_GROUP },
      () => randomCharacter(group),
    ),
  );
  const completePool = CHARACTER_GROUPS.join("");

  while (passwordCharacters.length < PASSWORD_LENGTH) {
    passwordCharacters.push(randomCharacter(completePool));
  }

  return secureShuffle(passwordCharacters);
}
