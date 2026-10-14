import { fireEvent, render } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { Avatar } from "./avatar";

describe("Avatar", () => {
  it("shows the image of the person", () => {
    const { container } = render(<Avatar name="ada" src="https://avatars.example.com/u/5" />);

    const avatar = container.querySelector("[data-slot=avatar]");

    expect(avatar?.tagName).toBe("IMG");
    expect(avatar).toHaveAttribute("src", "https://avatars.example.com/u/5");
  });

  it("shows the first letter of the name without an image, or when the image does not load", () => {
    const { container, rerender } = render(<Avatar name="ada" />);

    expect(container.querySelector("[data-slot=avatar]")).toHaveTextContent("A");

    rerender(<Avatar name="ada" src="https://avatars.example.com/u/5" />);
    const image = container.querySelector("img");
    if (image) {
      fireEvent.error(image);
    }

    expect(container.querySelector("img")).toBeNull();
    expect(container.querySelector("[data-slot=avatar]")).toHaveTextContent("A");
  });
});
