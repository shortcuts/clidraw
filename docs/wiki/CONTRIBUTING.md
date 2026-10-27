# Wiki Contributing Guide

## General writing guidelines

All wiki pages are read by **terminal users**. Write every sentence for a reader who has never seen the source code.

**Rules that apply to every page:**

- No source paths, module names, or Rust types. If you need to refer to something technical, describe what it does instead.
  - Bad: `canvas.rs` stores the undo history.
  - Good: clidraw keeps the undo history for each canvas.
- `<code>` is allowed for text the user types or reads verbatim: commands, flags, config keys, and file names.
  - Good: Run <code>clidraw list</code> to print your canvas names.
- `<kbd>` is allowed for key names: <kbd>Ctrl-p</kbd> opens the canvas switcher.
- Active voice, present tense. One idea per sentence.
- Audience test: could a user who never read the code understand every sentence? If not, rewrite.
- Do not name libraries or frameworks unless the page is about them.

---

## Page structure

### Content type

Every page is one of four types; keep each page to a single type.

- **Tutorial.** Learn by doing on a topic for the first time.
  - Structure: goal statement, step-by-step instructions, verifiable result after each step, final working outcome.
- **How-to guide.** Complete a specific task or solve a problem.
  - Structure: goal statement, prerequisites, numbered steps, expected result.
- **Reference.** Look up facts: commands, flags, keys, config options.
  - Structure: one child heading per entry, with a description and default below it; see "Reference pages".
- **Explanation.** Understand why a feature works the way it does.
  - Structure: context, core concept, alternatives and trade-offs, broader perspective.

Do not mix types on one page. If content fits multiple types, create separate pages and link between them.

### Screenshots

Screenshots are terminal captures. Put a screenshot in the section it illustrates. Wrap the section in `<section class="has-shot">` and place `<aside class="shot"><img class="term-shot" …></aside>` right after the section heading. On wide screens the screenshot floats in the right margin. On narrow screens it sits under the heading. Use one screenshot per section.

### Length and clarity

- Sentences must be under 20 words. One idea per sentence.
- Options, keys, and flags get one child heading each, or a bullet list for short items. Never a table, never paragraph form.
- Link to an explanation once, then reuse the link text or reference. Do not repeat the same explanation inline.
- Cut history, marketing tone ("powerful", "seamless"), and repeated phrasing. Every sentence must earn its place.

### Reference pages

Never use a `<table>`. Tables scroll sideways or squash on narrow terminals and in narrow windows. Give each entry a child heading instead.

Rules:
- Put one heading per entry, one level below the section heading (`<h3>` under `<h2>`, `<h4>` under `<h3>`). Add `class="item"` so the sidebar outline skips it.
- Give every item heading a unique `id`. Prefix it with the section `id` (e.g. `ctrl-p-canvases`).
- Write the heading as the entry name only: the command, flag, key, or option.
- Below the heading, write the description in plain sentences. Use a bullet list for steps or sub-points.
- End with `<p><strong>Default:</strong> …</p>` when the entry has a default. Use the same bold label for any other fact, such as `Shows when:`.
- Keep the same facts, in the same order, for every entry in a section.
- Use a plain bullet list for short name-to-value lookups, such as `<strong>10 m:</strong> 3 seconds`.
- Do not list entries twice. If each entry already has its own heading further down, link to those instead of adding an index.

Open a reference section with one sentence that says what the entries are. Add prose paragraphs only for behavior the entries cannot carry.

---

## For integrators

A wiki page may include one clearly labelled "For integrators" reference section, such as the command reference for scripts and agents. This section is an exception to the general rules above: it may contain JSON fields, command output formats, and exit codes. All other content on the page must follow the general writing guidelines.

Each such section must:
- Be headed with "(for integrators)" to set expectations.
- Open with a plain-language paragraph that explains what users need to know, linking to related features.
- Keep technical detail to commands, input and output formats, and error messages.

---

## Writing or editing wiki prose

Whenever a change adds or edits prose on a `docs/wiki/*.html` page, run the `/no-ai-slop`
skill on the page first, then the `/documentation` skill, before considering the page done.
`/no-ai-slop` cuts filler and AI-sounding phrasing while preserving meaning; `/documentation`
checks structure against the Diátaxis type rules in "Page structure" above. Judge
the result against the audience test above, not against either skill's own output in isolation.

This applies to prose edits only. Screenshot regeneration and nav wiring in `wiki-init.js` are unaffected.

---

## Navigation and page moves

### Sidebar structure

The sidebar is organized into five groups in fixed order:

- **Start.** First-time setup and getting going.
- **How-to.** Goal-based task pages.
- **Reference.** Lookup pages for commands, keys, and config options.
- **Explanation.** Pages that explain why features work the way they do.
- **About.** Meta pages for the project.

A group with no pages is not shown. Add a group only when a page needs one; do not add an empty group.

All nav items live in `docs/wiki/wiki-init.js`, in the `NAV_ITEMS` array. Each group is an object with a `group` name and an `items` array. See the current `NAV_ITEMS` at the top of `docs/wiki/wiki-init.js` for the live list and label wording.

### Adding a new page

1. Create `docs/wiki/<page>.html` with the same shell as `docs/wiki/index.html`: DOCTYPE, head with metadata and `style.css`, `<div class="layout"><main>…</main></div>`, and `<script src="wiki-init.js"></script>`.
2. Add `{ href: '<page>.html', label: '<Label>' }` to the `items` array of the group where it belongs. Use `&amp;` for ampersands in labels.
3. Add the page to `docs/wiki/sitemap.xml`.
4. No other file needs to be edited. `wiki-init.js` injects the sidebar into every page at runtime.

External links (GitHub, issue tracker) go in `EXT_ITEMS` below `NAV_ITEMS`, not in `NAV_ITEMS`.

### When to merge pages instead of adding a new one

Merge a feature into an existing page if:
- The feature is a variation or sub-topic of the existing page's subject, **or**
- The feature would have under roughly 300 words of content on its own.

Add a new page only if the feature is a distinct user goal that does not fit an existing group item.

### Redirect stubs for moved or merged pages

When a page is moved or merged into another:

1. Keep the old file as a redirect stub.
2. Remove the old file from `NAV_ITEMS` and from `sitemap.xml`.
3. The stub includes:
   - A `<title>` matching the new location.
   - A `<meta http-equiv="refresh">` redirecting to the new page's relative URL.
   - A `<link rel="canonical">` pointing to `https://clidraw.shrtcts.fr/<new-page>.html`.
   - A `<meta name="robots" content="noindex">` to exclude it from search.
   - One plain-text link in the `<body>`: "This page moved. See <a href="...">Page Name</a>."

Stubs are never deleted. Old links from outside the wiki still resolve.

---

## SEO basics

Every real page (not a redirect stub) must have:

### Meta tags

- **Title:** `<title>` of the form "<Page name> | clidraw". Must be unique per page.
- **Description:** A unique `<meta name="description">` containing one sentence under 160 characters in plain language.
- **Open Graph and Twitter:** Match title and description to `og:title`, `og:description`, `twitter:title`, `twitter:description`.
- **Canonical and URL:** `<link rel="canonical">` pointing to `https://clidraw.shrtcts.fr/<page>.html`. Set `og:url` to the same canonical URL.

### Headings and IDs

- Every page has exactly one `<h1>` with a unique `id` attribute.
- Every `<h2>` and `<h3>` has a unique `id`. The sidebar outline uses these IDs to build the page's table of contents.
- Use descriptive IDs matching the heading text (e.g. `id="canvas-switcher"` for "Canvas switcher").

### Links and images

- Use descriptive link text. Never "click here" or "more".
- Link to sibling pages using relative `href`s (e.g. `href="map.html"`, not full URLs).
- Link each related page once per section.
- Every image must have `alt` text describing its content.

### Sitemap

- Add new real pages to `sitemap.xml` with `<loc>https://clidraw.shrtcts.fr/<page>.html</loc>`.
- Do not list redirect stubs in `sitemap.xml`.
