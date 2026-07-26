const search = document.querySelector(".site-search");
const input = search?.querySelector("input[type='search']");
const cards = [...document.querySelectorAll("[data-search-card]")];
const status = document.querySelector("[data-search-status]");

// GitHub Pages rewrites source-relative Markdown links during its Jekyll build.
// Keep a local `jekyll build` preview navigable when that default plugin is not
// installed by resolving links against the page's source directory.
for (const anchor of document.querySelectorAll("a[href]")) {
  const href = anchor.getAttribute("href");
  if (!href || /^[a-z]+:/i.test(href) || !href.match(/\.md(?:#.*)?$/)) {
    continue;
  }
  const sourceDirectory = new URL("../", window.location.href);
  const target = new URL(href, sourceDirectory);
  target.pathname = target.pathname.replace(/\.md$/, "/");
  anchor.href = target.href;
}

function applySearch(value) {
  if (!cards.length) {
    return false;
  }
  const terms = value
    .toLocaleLowerCase()
    .trim()
    .split(/\s+/)
    .filter(Boolean);
  let matches = 0;
  for (const card of cards) {
    const haystack = card.textContent.toLocaleLowerCase();
    const visible = terms.every((term) => haystack.includes(term));
    card.hidden = !visible;
    matches += Number(visible);
  }
  if (status) {
    status.textContent = terms.length
      ? `${matches} documentation ${matches === 1 ? "topic" : "topics"} found`
      : "Browse by language or compiler topic";
  }
  return true;
}

const initialQuery = new URLSearchParams(window.location.search).get("q") ?? "";
if (input && initialQuery) {
  input.value = initialQuery;
  applySearch(initialQuery);
}
input?.addEventListener("input", () => applySearch(input.value));
search?.addEventListener("submit", (event) => {
  if (applySearch(input.value)) {
    event.preventDefault();
    window.history.replaceState(
      {},
      "",
      input.value ? `?q=${encodeURIComponent(input.value)}` : window.location.pathname,
    );
  }
});
