import { expect, test } from "bun:test";
import { composeKeypress, createComposerState } from "./composer";

test("composer accepts astral text and erases a whole grapheme", () => {
  let state = createComposerState();
  for (const key of ["界", "😀", "e", "\u0301"]) state = composeKeypress(state, key).state;
  expect(state.draft).toBe("界😀é");
  state = composeKeypress(state, "backspace").state;
  expect(state.draft).toBe("界😀");
  state = composeKeypress(state, "backspace").state;
  expect(state.draft).toBe("界");
  expect(composeKeypress(state, "ctrl+c").state.draft).toBe("界");
});
