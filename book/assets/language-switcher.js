// Per-page language switcher for the bilingual Keleusma guide.
//
// The book is a single English source translated through gettext. The English
// edition deploys at the book root and the Japanese edition at the `ja/`
// subpath, with identical page file names in both. This script inserts a
// toggle into the mdbook header that links the current page to its counterpart
// in the other language, computed from the URL alone so it works on every page
// without server support.
(function () {
  "use strict";

  // The book is flat (every page sits directly under the book root), so the
  // language segment, when present, is the path segment just before the page
  // file name. Toggling adds or removes that single `ja` segment.
  function pathSegments() {
    return window.location.pathname.split("/");
  }

  function currentIsJapanese() {
    var segs = pathSegments();
    // segs[last] is the page file name (or "" for a directory URL); the
    // language marker, if any, is the segment immediately before it.
    return segs[segs.length - 2] === "ja";
  }

  function counterpartHref() {
    var segs = pathSegments();
    var last = segs.length - 1;
    if (segs[last - 1] === "ja") {
      segs.splice(last - 1, 1); // Japanese -> English: drop the `ja` segment.
    } else {
      segs.splice(last, 0, "ja"); // English -> Japanese: insert `ja` before the page.
    }
    return segs.join("/") + window.location.hash;
  }

  function buildToggle() {
    var a = document.createElement("a");
    a.className = "language-toggle";
    a.href = counterpartHref();
    // Label with the language the reader will switch TO.
    a.textContent = currentIsJapanese() ? "English" : "日本語";
    var title = "Switch language / 言語切り替え";
    a.title = title;
    a.setAttribute("aria-label", title);
    a.style.margin = "0 8px";
    a.style.fontWeight = "600";
    a.style.textDecoration = "none";
    a.style.whiteSpace = "nowrap";
    return a;
  }

  function inject() {
    if (document.querySelector(".language-toggle")) {
      return; // Idempotent: never insert twice.
    }
    var host =
      document.querySelector("#menu-bar .right-buttons") ||
      document.querySelector("#menu-bar .left-buttons") ||
      document.querySelector("#menu-bar");
    if (host) {
      host.appendChild(buildToggle());
    }
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", inject);
  } else {
    inject();
  }
})();
