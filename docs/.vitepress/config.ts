import { defineConfig } from "vitepress";

const base = process.env.PAGES_BASE ?? "/";

export default defineConfig({
  lang: "en-US",
  title: "Zrimo",
  titleTemplate: ":title · Zrimo",
  description:
    "Private, embeddable document viewing for PDF, Office, images and data — entirely in the browser.",
  base,
  cleanUrls: true,
  lastUpdated: true,
  srcExclude: ["testing/**", "universal-document-viewer/**"],
  ignoreDeadLinks: [/\/demo(?:\/index)?$/],
  head: [
    ["link", { rel: "icon", type: "image/svg+xml", href: `${base}logo.svg` }],
    ["meta", { name: "theme-color", content: "#101828" }],
    ["meta", { property: "og:type", content: "website" }],
    ["meta", { property: "og:title", content: "Zrimo document viewer" }],
    [
      "meta",
      {
        property: "og:description",
        content: "Any document. One canvas. No uploads.",
      },
    ],
  ],
  themeConfig: {
    logo: { src: "/logo.svg", alt: "Zrimo" },
    nav: [
      { text: "Guide", link: "/getting-started" },
      { text: "API", link: "/api/reference" },
      { text: "Formats", link: "/compatibility" },
      { text: "React demo", link: "/demo/" },
    ],
    sidebar: [
      {
        text: "Start",
        items: [
          { text: "Getting started", link: "/getting-started" },
          { text: "Framework integrations", link: "/integrations" },
          {
            text: "Migration from udoc-viewer",
            link: "/migration-from-udoc-viewer",
          },
        ],
      },
      {
        text: "API and UI",
        items: [
          { text: "API reference", link: "/api/reference" },
          { text: "Headless API", link: "/api/headless" },
          { text: "Runtime and lifecycle", link: "/api/runtime" },
          { text: "Built-in UI", link: "/ui" },
        ],
      },
      {
        text: "Formats",
        items: [
          { text: "Compatibility", link: "/compatibility" },
          { text: "Office", link: "/formats/office" },
          { text: "PDF, images and data", link: "/formats/pdf-images-data" },
          { text: "Fonts and languages", link: "/fonts" },
        ],
      },
      {
        text: "Operations",
        items: [
          { text: "Architecture", link: "/architecture" },
          { text: "Performance", link: "/performance" },
          { text: "Security", link: "/security" },
          { text: "Troubleshooting", link: "/troubleshooting" },
        ],
      },
    ],
    socialLinks: [{ icon: "github", link: "https://github.com/bnku/zimo" }],
    editLink: {
      pattern: "https://github.com/bnku/zimo/edit/main/docs/:path",
      text: "Edit this page on GitHub",
    },
    search: { provider: "local" },
    footer: {
      message: "Released under the MIT or Apache-2.0 license.",
      copyright: "Copyright © 2026 Zrimo contributors",
    },
  },
});
