import { expect, test } from "@playwright/test";

const fontCorpus = [
  "pdfjs-mmtype1.pdf",
  "pdfjs-arabic-cid-truetype.pdf",
  "pdfjs-cff-cid.pdf",
  "pdfjs-standard-fonts.pdf",
  "pdfjs-noembed-jis7.pdf",
  "pdfjs-complex-truetype.pdf",
] as const;

test("PDF.js renders the font matrix without rectangle fallback or external network", async ({
  page,
}) => {
  const requests: string[] = [];
  const consoleMessages: string[] = [];
  page.on("request", (request) => requests.push(request.url()));
  page.on("console", (message) => consoleMessages.push(message.text()));
  await page.goto("/");
  const results = await page.evaluate(async (fileNames) => {
    const { ViewerClient } = (await import("/main.js")) as any;
    const client = ViewerClient.create({
      assetBaseUrl: new URL("/", location.href),
    });
    const output: Array<{
      fileName: string;
      width: number;
      height: number;
      darkPixels: number;
      solidRectangles: number;
      textLength: number;
    }> = [];
    for (const fileName of fileNames) {
      const viewer = client.createViewer();
      const bytes = new Uint8Array(
        await (await fetch(`/corpus/${fileName}`)).arrayBuffer(),
      );
      await viewer.load(bytes, { fileName });
      const canvas = document.createElement("canvas");
      await viewer.renderPage(0, canvas, {
        zoom: 1,
        devicePixelRatio: 1,
      });
      const pixels = canvas
        .getContext("2d")!
        .getImageData(0, 0, canvas.width, canvas.height);
      let darkPixels = 0;
      for (let offset = 0; offset < pixels.data.length; offset += 4)
        if (
          pixels.data[offset]! < 225 ||
          pixels.data[offset + 1]! < 225 ||
          pixels.data[offset + 2]! < 225
        )
          darkPixels += 1;
      output.push({
        fileName,
        width: canvas.width,
        height: canvas.height,
        darkPixels,
        solidRectangles: countSolidBlackRectangles(pixels),
        textLength: (await viewer.getPageText(0)).length,
      });
      await viewer.destroy();
    }
    await client.destroy();
    return output;

    function countSolidBlackRectangles(image: ImageData): number {
      const { width, height, data } = image;
      const visited = new Uint8Array(width * height);
      let rectangles = 0;
      for (let start = 0; start < visited.length; start += 1) {
        if (visited[start] || !isBlack(start)) continue;
        const stack = [start];
        visited[start] = 1;
        let pixels = 0;
        let left = width;
        let right = 0;
        let top = height;
        let bottom = 0;
        while (stack.length > 0) {
          const current = stack.pop()!;
          const x = current % width;
          const y = Math.floor(current / width);
          pixels += 1;
          left = Math.min(left, x);
          right = Math.max(right, x);
          top = Math.min(top, y);
          bottom = Math.max(bottom, y);
          for (const next of [
            x > 0 ? current - 1 : -1,
            x + 1 < width ? current + 1 : -1,
            y > 0 ? current - width : -1,
            y + 1 < height ? current + width : -1,
          ]) {
            if (next >= 0 && !visited[next] && isBlack(next)) {
              visited[next] = 1;
              stack.push(next);
            }
          }
        }
        const boxWidth = right - left + 1;
        const boxHeight = bottom - top + 1;
        if (
          boxWidth >= 6 &&
          boxHeight >= 8 &&
          boxWidth * boxHeight >= 80 &&
          pixels / (boxWidth * boxHeight) >= 0.9
        )
          rectangles += 1;
      }
      return rectangles;

      function isBlack(pixel: number): boolean {
        const offset = pixel * 4;
        return (
          data[offset]! < 24 &&
          data[offset + 1]! < 24 &&
          data[offset + 2]! < 24 &&
          data[offset + 3]! > 240
        );
      }
    }
  }, fontCorpus);

  expect(results).toHaveLength(fontCorpus.length);
  for (const result of results) {
    expect(result.width, result.fileName).toBeGreaterThan(20);
    expect(result.height, result.fileName).toBeGreaterThan(20);
    expect(result.darkPixels, result.fileName).toBeGreaterThan(20);
    expect(result.solidRectangles, result.fileName).toBeLessThan(3);
  }
  expect(
    results.find((result) => result.fileName.includes("arabic"))?.textLength,
  ).toBeGreaterThan(0);

  const urls = requests.map((request) => new URL(request));
  expect(new Set(urls.map((url) => url.host))).toEqual(
    new Set(["127.0.0.1:4173"]),
  );
  expect(
    urls.some((url) => url.pathname === "/workers/pdf.worker.min.mjs"),
  ).toBe(true);
  expect(
    urls.some((url) =>
      url.pathname.startsWith("/assets/pdfjs/standard_fonts/"),
    ),
  ).toBe(true);
  expect(
    urls.some((url) => url.pathname.startsWith("/assets/pdfjs/cmaps/")),
  ).toBe(true);
  expect(
    consoleMessages.filter((message) =>
      message.includes("Math.sumPrecise is not a function"),
    ),
  ).toEqual([]);
});

test("PDF.js render cancellation stays typed during destroy", async ({
  page,
}) => {
  await page.goto("/");
  const code = await page.evaluate(async () => {
    const { ViewerClient } = (await import("/main.js")) as any;
    const client = ViewerClient.create({
      assetBaseUrl: new URL("/", location.href),
    });
    const viewer = client.createViewer();
    const bytes = new Uint8Array(
      await (await fetch("/corpus/pdfjs-standard-fonts.pdf")).arrayBuffer(),
    );
    await viewer.load(bytes, { fileName: "pdfjs-standard-fonts.pdf" });
    const rendering = viewer
      .renderPage(0, document.createElement("canvas"), {
        zoom: 4,
        devicePixelRatio: 2,
      })
      .then(
        () => "completed",
        (error: { code?: string }) => error.code ?? "unknown",
      );
    await viewer.destroy();
    const result = await rendering;
    await client.destroy();
    return result;
  });

  expect(["aborted", "completed"]).toContain(code);
});

test("PDF.js renders documents with link annotations", async ({ page }) => {
  const messages: string[] = [];
  page.on("console", (message) => messages.push(message.text()));
  await page.goto("/");
  const bytes = [...createLinkAnnotationPdf()];
  const result = await page.evaluate(async (input) => {
    const { ViewerClient } = (await import("/main.js")) as any;
    const client = ViewerClient.create({
      assetBaseUrl: new URL("/", location.href),
    });
    const viewer = client.createViewer();
    await viewer.load(new Uint8Array(input), { fileName: "link.pdf" });
    const canvas = document.createElement("canvas");
    await viewer.renderPage(0, canvas, { zoom: 1, devicePixelRatio: 1 });
    const image = canvas
      .getContext("2d")!
      .getImageData(0, 0, canvas.width, canvas.height).data;
    let darkPixels = 0;
    for (let offset = 0; offset < image.length; offset += 4)
      if (image[offset]! < 220) darkPixels += 1;
    const text = await viewer.getPageText(0);
    await viewer.destroy();
    await client.destroy();
    return { darkPixels, text };
  }, bytes);

  expect(result.darkPixels).toBeGreaterThan(10);
  expect(result.text).toContain("Linked page");
  expect(messages.filter((message) => message.includes("sumPrecise"))).toEqual(
    [],
  );
});

function createLinkAnnotationPdf(): Uint8Array {
  const encoder = new TextEncoder();
  const content = "BT /F1 14 Tf 20 100 Td (Linked page) Tj ET";
  const objects = [
    "<< /Type /Catalog /Pages 2 0 R >>",
    "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R /Annots [6 0 R] >>",
    `<< /Length ${content.length} >>\nstream\n${content}\nendstream`,
    "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
    "<< /Type /Annot /Subtype /Link /Rect [15 90 120 120] /Border [0 0 0] /A << /S /URI /URI (https://example.com/) >> >>",
  ];
  let body = "%PDF-1.7\n%synthetic\n";
  const offsets = [0];
  for (let index = 0; index < objects.length; index += 1) {
    offsets.push(encoder.encode(body).byteLength);
    body += `${index + 1} 0 obj\n${objects[index]}\nendobj\n`;
  }
  const xref = encoder.encode(body).byteLength;
  body += `xref\n0 ${objects.length + 1}\n`;
  body += "0000000000 65535 f \n";
  for (const offset of offsets.slice(1))
    body += `${String(offset).padStart(10, "0")} 00000 n \n`;
  body += `trailer\n<< /Size ${objects.length + 1} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
  return encoder.encode(body);
}
