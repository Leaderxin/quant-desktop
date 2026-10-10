// QuantDesktop 官网 — progressive enhancement only.
//
// Nothing here is load-bearing: with JavaScript disabled the page still renders
// complete and readable (styles.css only hides .reveal elements when the inline
// head script has marked the document as .js). No network calls, so the page is
// equally intact offline and behind a slow connection.

(function () {
  'use strict';

  /* ── Theme ───────────────────────────────────────────────────────
     The attribute is already set by the inline script in <head>; this only
     handles flipping it and keeping the browser chrome in step. */
  var STORAGE_KEY = 'qd-theme';
  var root = document.documentElement;

  function storedTheme() {
    try { return localStorage.getItem(STORAGE_KEY); } catch (e) { return null; }
  }

  function storeTheme(theme) {
    try { localStorage.setItem(STORAGE_KEY, theme); } catch (e) { /* private mode */ }
  }

  function systemTheme() {
    return window.matchMedia && window.matchMedia('(prefers-color-scheme: light)').matches
      ? 'light'
      : 'dark';
  }

  function applyTheme(theme) {
    root.setAttribute('data-theme', theme);
    var meta = document.querySelector('meta[name="theme-color"]');
    if (meta) meta.setAttribute('content', theme === 'light' ? '#ffffff' : '#0d1117');
  }

  var themeToggle = document.getElementById('themeToggle');
  if (themeToggle) {
    themeToggle.addEventListener('click', function () {
      var next = root.getAttribute('data-theme') === 'light' ? 'dark' : 'light';
      applyTheme(next);
      storeTheme(next);
    });
  }

  // Keep following the OS, but only until the visitor picks a side themselves —
  // an explicit choice must not be silently overridden by a sunset.
  if (window.matchMedia) {
    var mq = window.matchMedia('(prefers-color-scheme: light)');
    var followSystem = function () {
      if (!storedTheme()) applyTheme(systemTheme());
    };
    if (mq.addEventListener) mq.addEventListener('change', followSystem);
    else if (mq.addListener) mq.addListener(followSystem);
  }

  applyTheme(root.getAttribute('data-theme') || systemTheme());

  /* ── Nav ─────────────────────────────────────────────────────────
     Transparent over the hero, blurred once the page scrolls under it. */
  var nav = document.getElementById('nav');

  function syncNav() {
    if (nav) nav.classList.toggle('is-scrolled', window.scrollY > 8);
  }

  window.addEventListener('scroll', syncNav, { passive: true });
  syncNav();

  /* ── Mobile menu ─────────────────────────────────────────────── */
  var burger = document.getElementById('navBurger');
  var navLinks = document.getElementById('navLinks');

  function closeMenu() {
    if (!navLinks || !burger) return;
    navLinks.classList.remove('is-open');
    burger.setAttribute('aria-expanded', 'false');
  }

  if (burger && navLinks) {
    burger.addEventListener('click', function () {
      var open = navLinks.classList.toggle('is-open');
      burger.setAttribute('aria-expanded', open ? 'true' : 'false');
    });

    // Any anchor tap navigates within the page, so the sheet has to go away —
    // otherwise it covers the section it just scrolled to.
    navLinks.addEventListener('click', function (e) {
      if (e.target.closest && e.target.closest('a')) closeMenu();
    });

    document.addEventListener('keydown', function (e) {
      if (e.key === 'Escape') closeMenu();
    });
  }

  /* ── Scroll reveal ───────────────────────────────────────────── */
  var revealables = document.querySelectorAll('.reveal');
  var prefersReduced =
    window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches;

  if (prefersReduced || !('IntersectionObserver' in window)) {
    // Without this branch a missing observer would leave every .reveal element
    // stuck at opacity 0 — the whole page below the hero, invisible.
    Array.prototype.forEach.call(revealables, function (el) {
      el.classList.add('is-visible');
    });
  } else {
    var observer = new IntersectionObserver(
      function (entries) {
        entries.forEach(function (entry) {
          if (!entry.isIntersecting) return;
          entry.target.classList.add('is-visible');
          observer.unobserve(entry.target);
        });
      },
      { rootMargin: '0px 0px -8% 0px', threshold: 0.05 }
    );

    Array.prototype.forEach.call(revealables, function (el) {
      observer.observe(el);
    });
  }
})();
