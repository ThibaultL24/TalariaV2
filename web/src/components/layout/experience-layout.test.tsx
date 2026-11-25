// web/src/components/layout/experience-layout.test.tsx
import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { expect, test, vi } from "vitest";
import { ExperienceLayout } from "./experience-layout";

vi.mock("@/components/layout/navbar", () => ({ Navbar: () => <nav>Nav</nav> }));
vi.mock("@/components/layout/experience-navigation", () => ({ ExperienceNavigation: () => <nav>Experiences</nav> }));
vi.mock("@/components/layout/experience-subnav", () => ({ ExperienceSubnav: () => null }));
vi.mock("@/components/layout/experience-demo-figures", () => ({ ExperienceDemoFigures: () => null }));
vi.mock("@/components/entity/entity-header", () => ({ EntityHeader: () => null }));

test("the mobile tools button exposes its expanded state to the keyboard", () => {
  let open = false;
  const view = render(
    <MemoryRouter>
      <ExperienceLayout
        experience="poreia"
        entityId="person-1"
        label="Napoléon"
        immersive
        mobileStage
        toolsOpen={open}
        onToggleTools={() => {
          open = !open;
          view.rerender(
            <MemoryRouter>
              <ExperienceLayout
                experience="poreia"
                entityId="person-1"
                label="Napoléon"
                immersive
                mobileStage
                toolsOpen={open}
                onToggleTools={() => undefined}
                showHeroBand={false}
                search={<p>Search</p>}
              >
                <p>Sheet</p>
              </ExperienceLayout>
            </MemoryRouter>,
          );
        }}
        showHeroBand={false}
        search={<p>Search</p>}
      >
        <p>Sheet</p>
      </ExperienceLayout>
    </MemoryRouter>,
  );
  const button = screen.getByRole("button");
  expect(button.getAttribute("aria-expanded")).toBe("false");
  fireEvent.click(button);
  expect(screen.getByRole("button").getAttribute("aria-expanded")).toBe("true");
});
