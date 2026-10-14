import { describe, expect, it } from "vitest";

import { deviceAddress, userCodeOf } from "./device";

describe("the code of the command line", () => {
  it("reads whatever its case, its spaces and its dash", () => {
    for (const typed of ["WDJB-MJHT", "wdjbmjht", " wdjb mjht ", "Wd-Jb-Mj-Ht"]) {
      expect(userCodeOf(typed)).toBe("WDJB-MJHT");
    }
  });

  it("refuses what cannot be a code", () => {
    for (const typed of ["", "WDJB-MJH", "WDJB-MJHTX", "WAJB-MJHT", "WDJB-MJH1"]) {
      expect(userCodeOf(typed)).toBeNull();
    }
  });

  it("comes back to its page after the sign-in", () => {
    expect(deviceAddress("WDJB-MJHT")).toBe("/device?code=WDJB-MJHT");
    expect(deviceAddress(undefined)).toBe("/device");
  });
});
