(function () {
  "use strict";

  var MOBILE_BREAKPOINT = 768;
  var body = document.body;
  var hamburgerBtn = document.getElementById("hamburger-btn");
  var drawer = document.getElementById("mobile-stories-drawer");
  var backdrop = document.getElementById("mobile-drawer-backdrop");
  var tocToggle = document.getElementById("mobile-toc-toggle");
  var tocList = document.getElementById("mobile-toc-list");

  /* -- Drawer -- */
  function openDrawer() {
    drawer.classList.add("is-open");
    hamburgerBtn.classList.add("is-open");
    hamburgerBtn.setAttribute("aria-expanded", "true");
    drawer.setAttribute("aria-hidden", "false");
    document.documentElement.style.overflow = "hidden";
  }

  function closeDrawer() {
    drawer.classList.remove("is-open");
    hamburgerBtn.classList.remove("is-open");
    hamburgerBtn.setAttribute("aria-expanded", "false");
    drawer.setAttribute("aria-hidden", "true");
    document.documentElement.style.overflow = "";
    if (tocList) {
      tocList.classList.remove("is-open");
      tocToggle.classList.remove("is-open");
      tocToggle.setAttribute("aria-expanded", "false");
    }
  }
  if (hamburgerBtn) {
    hamburgerBtn.addEventListener("click", function () {
      drawer.classList.contains("is-open") ? closeDrawer() : openDrawer();
    });
  }

  if (backdrop) {
    backdrop.addEventListener("click", closeDrawer);
  }

  document.addEventListener("keydown", function (e) {
    if (e.key === "Escape") closeDrawer();
  });

  if (drawer) {
    drawer.addEventListener("click", function (e) {
      var link = e.target.closest("a[href]");
      if (link && !link.href.includes("#")) closeDrawer();
    });
  }

  /* -- TOC toggle -- */
  if (tocToggle && tocList) {
    function toggleToc() {
      var isOpen = tocList.classList.toggle("is-open");
      tocToggle.classList.toggle("is-open", isOpen);
      tocToggle.setAttribute("aria-expanded", String(isOpen));
    }
    tocToggle.addEventListener("click", toggleToc);
    tocToggle.addEventListener("keydown", function (e) {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        toggleToc();
      }
    });
    tocList.addEventListener("click", function (e) {
      if (e.target.closest('a[href^="#"]')) setTimeout(closeDrawer, 120);
    });
  }

  /* -- Mobile class -- */
  function applyMobile(on) {
    body.classList.toggle("is-mobile", on);
    if (!on && drawer) closeDrawer();
  }

  /* -- Resize -- */
  var resizeTimer;
  window.addEventListener("resize", function () {
    clearTimeout(resizeTimer);
    resizeTimer = setTimeout(function () {
      applyMobile(window.innerWidth <= MOBILE_BREAKPOINT);
    }, 100);
  });
  window.addEventListener("orientationchange", function () {
    window.dispatchEvent(new Event("resize"));
  });

  /* -- Init -- */
  // server-side detection is written to data-server-mobile on <body>
  var serverSaidMobile = body.dataset.serverMobile === "true";
  applyMobile(serverSaidMobile || window.innerWidth <= MOBILE_BREAKPOINT);
})();
