import { describe, expect, it } from "bun:test";
import { signalName } from "./signal-code";

describe("signalName", () => {
  it("passes names through, stringifies numbers, maps absence to null", () => {
    expect(signalName("SIGTERM")).toBe("SIGTERM");
    expect(signalName(15)).toBe("15");
    expect(signalName(null)).toBeNull();
    expect(signalName(undefined)).toBeNull();
  });
});
