import type { Page } from "@playwright/test";

const OFFICE_FAMILIES = ["Arial", "Calibri", "Cambria", "Times New Roman"];

export async function installDeterministicOfficeFonts(
  page: Page,
): Promise<void> {
  await page.evaluate(async (families) => {
    const source =
      "url('/fonts/noto-sans-latin-cyrillic.woff2') format('woff2')";
    const faces = await Promise.all(
      families.map((family) =>
        new FontFace(family, source, {
          style: "normal",
          weight: "400",
        }).load(),
      ),
    );
    for (const face of faces) document.fonts.add(face);
    await document.fonts.ready;
  }, OFFICE_FAMILIES);
}
