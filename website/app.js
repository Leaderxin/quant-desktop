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

  /* ── Hero carousel ───────────────────────────────────────────────
     No autoplay on purpose: a hero that moves on its own competes with the
     copy next to it, and it is exactly the motion prefers-reduced-motion
     exists to stop. Everything here is manual. */
  var carousel = document.getElementById('heroCarousel');
  var track = document.getElementById('carouselTrack');

  if (carousel && track) {
    var slides = Array.prototype.slice.call(track.children);
    var prevBtn = document.getElementById('carouselPrev');
    var nextBtn = document.getElementById('carouselNext');
    var dots = Array.prototype.slice.call(document.querySelectorAll('.carousel__dot'));
    var indexEl = document.getElementById('carouselIndex');
    var textEl = document.getElementById('carouselText');
    var current = 0;

    // Slide 1's caption is whatever is already in the DOM — that is the no-JS
    // fallback — so only slides 2+ carry data-caption and no sentence is
    // written twice.
    var captions = slides.map(function (slide, i) {
      return slide.getAttribute('data-caption') || (i === 0 && textEl ? textEl.textContent : '');
    });

    function goTo(next) {
      var count = slides.length;
      current = ((next % count) + count) % count;
      track.style.transform = 'translateX(' + -current * 100 + '%)';

      slides.forEach(function (slide, i) {
        // The other slides are only translated out of view, so they are still
        // in the accessibility tree — without this a screen reader reads all
        // five screenshots at once.
        slide.setAttribute('aria-hidden', i === current ? 'false' : 'true');
      });

      dots.forEach(function (dot, i) {
        if (i === current) dot.setAttribute('aria-current', 'true');
        else dot.removeAttribute('aria-current');
      });

      if (indexEl) indexEl.textContent = (current < 9 ? '0' : '') + (current + 1);
      if (textEl && captions[current]) textEl.textContent = captions[current];
    }

    if (prevBtn) prevBtn.addEventListener('click', function () { goTo(current - 1); });
    if (nextBtn) nextBtn.addEventListener('click', function () { goTo(current + 1); });

    dots.forEach(function (dot, i) {
      dot.addEventListener('click', function () { goTo(i); });
    });

    // Arrow keys, but only while focus is inside the carousel — a listener on
    // document would hijack the page's own arrow-key scrolling.
    carousel.addEventListener('keydown', function (e) {
      if (e.key === 'ArrowLeft') {
        goTo(current - 1);
        e.preventDefault();
      } else if (e.key === 'ArrowRight') {
        goTo(current + 1);
        e.preventDefault();
      }
    });

    // Swipe is a bonus, never the only way in (the arrows and dots are always
    // visible). Strictly guarded and deliberately without preventDefault: a
    // vertical drag still scrolls the page, and a mostly-vertical one is never
    // mistaken for a slide change.
    var startX = 0;
    var startY = 0;
    var tracking = false;

    carousel.addEventListener('touchstart', function (e) {
      if (e.touches.length !== 1) { tracking = false; return; }
      startX = e.touches[0].clientX;
      startY = e.touches[0].clientY;
      tracking = true;
    }, { passive: true });

    carousel.addEventListener('touchend', function (e) {
      if (!tracking) return;
      tracking = false;
      var dx = e.changedTouches[0].clientX - startX;
      var dy = e.changedTouches[0].clientY - startY;
      if (Math.abs(dx) > 45 && Math.abs(dx) > Math.abs(dy) * 3) {
        goTo(dx < 0 ? current + 1 : current - 1);
      }
    }, { passive: true });

    goTo(0);

    // Slides 2–5 are loading="lazy" to keep the first paint light. But an image
    // that is merely translated out of view is never "near the viewport", so it
    // would not start loading until the first switch — and that switch would
    // show a blank frame. Warm the cache once the page is idle.
    function warmSlides() {
      slides.forEach(function (slide) {
        var img = slide.querySelector('img');
        if (img) new Image().src = img.src;
      });
    }

    if ('requestIdleCallback' in window) {
      window.requestIdleCallback(warmSlides, { timeout: 3000 });
    } else {
      window.addEventListener('load', warmSlides);
    }
  }

  /* ── Back to top ─────────────────────────────────────────────────
     The scrolling itself is the anchor's job (html has scroll-behavior:
     smooth). This only decides when the button is worth showing, and drives
     the progress ring. */
  var toTop = document.getElementById('toTop');

  if (toTop) {
    var ring = document.getElementById('toTopBar');
    var circumference = 0;
    var maxScroll = 0;

    if (ring) {
      circumference = 2 * Math.PI * ring.r.baseVal.value;
      ring.style.strokeDasharray = circumference;
      ring.style.strokeDashoffset = circumference;
    }

    // scrollHeight is a layout-dependent read, so it is measured here instead
    // of inside the scroll handler — scrolling never forces a reflow. That is
    // only sound because every image on the page reserves its aspect ratio,
    // so the document stops changing height once it has loaded.
    function measure() {
      maxScroll = Math.max(0, document.documentElement.scrollHeight - window.innerHeight);
    }

    function syncToTop() {
      // Only after the reader has committed to scrolling; a button that shows
      // up straight away is just chrome.
      toTop.classList.toggle('is-visible', window.scrollY > window.innerHeight * 0.75);

      if (ring && maxScroll > 0) {
        var progress = Math.min(1, Math.max(0, window.scrollY / maxScroll));
        ring.style.strokeDashoffset = circumference * (1 - progress);
      }
    }

    measure();
    syncToTop();

    window.addEventListener('scroll', syncToTop, { passive: true });
    window.addEventListener('resize', function () { measure(); syncToTop(); }, { passive: true });
    // Re-measure once everything has loaded: the first pass runs before the
    // lazy images below the fold have settled.
    window.addEventListener('load', function () { measure(); syncToTop(); });
  }
})();
