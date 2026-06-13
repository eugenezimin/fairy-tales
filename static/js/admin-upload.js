(function () {
  "use strict";

  var panel = document.getElementById("admin-panel");
  if (!panel) return;

  var dropzone = document.getElementById("admin-dropzone");
  var fileInput = document.getElementById("admin-file");
  var statusBar = document.getElementById("admin-status-bar");
  var statusIcon = document.getElementById("admin-status-icon");
  var statusText = document.getElementById("admin-status");
  var submitBtn = document.getElementById("admin-submit");
  var closeBtn = document.getElementById("admin-close");
  var trigger = document.getElementById("upload-trigger");

  var chosenMd = null;
  var chosenImages = [];

  // ── Status helpers ──────────────────────────────────────────────────────

  function setStatus(msg, type) {
    // type: 'info' | 'error' | 'success' | '' (hidden)
    statusBar.className = "";
    if (!msg) {
      statusBar.style.display = "none";
      return;
    }
    statusBar.style.display = "flex";
    statusBar.classList.add("visible", "status-" + (type || "info"));

    var icons = { error: "✗", success: "✓", info: "ℹ" };
    statusIcon.textContent = icons[type] || "";
    statusText.textContent = msg;
  }

  // ── File list rendering ─────────────────────────────────────────────────

  function formatSize(bytes) {
    if (bytes < 1024) return bytes + " B";
    if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + " KB";
    return (bytes / (1024 * 1024)).toFixed(1) + " MB";
  }

  function renderFileList() {
    var list = document.getElementById("admin-file-list");
    if (!list) return;
    list.innerHTML = "";

    var hasFiles = chosenMd || chosenImages.length > 0;
    dropzone.classList.toggle("has-files", hasFiles);

    if (chosenMd) {
      var li = document.createElement("li");
      li.className = "file-md";
      li.innerHTML =
        '<span class="file-icon">📄</span>' +
        '<span class="file-name">' +
        escHtml(chosenMd.name) +
        "</span>" +
        '<span class="file-size">' +
        formatSize(chosenMd.size) +
        "</span>";
      list.appendChild(li);
    }

    chosenImages.forEach(function (img) {
      var li = document.createElement("li");
      li.className = "file-img";
      li.innerHTML =
        '<span class="file-icon">🖼</span>' +
        '<span class="file-name">' +
        escHtml(img.file.name) +
        "</span>" +
        '<span class="file-size">' +
        formatSize(img.file.size) +
        "</span>";
      list.appendChild(li);
    });
  }

  function escHtml(str) {
    return str.replace(/[&<>"']/g, function (c) {
      return {
        "&": "&amp;",
        "<": "&lt;",
        ">": "&gt;",
        '"': "&quot;",
        "'": "&#39;",
      }[c];
    });
  }

  // ── Panel lifecycle ─────────────────────────────────────────────────────

  function resetPanel() {
    setStatus("");
    fileInput.value = "";
    chosenMd = null;
    chosenImages = [];
    submitBtn.disabled = true;
    renderFileList();
  }

  if (trigger) {
    trigger.addEventListener("click", function (e) {
      e.preventDefault();
      resetPanel();
      panel.classList.add("is-open");
    });
  }

  closeBtn.addEventListener("click", function () {
    panel.classList.remove("is-open");
  });
  panel.addEventListener("click", function (e) {
    if (e.target === panel) panel.classList.remove("is-open");
  });

  // ── File intake ─────────────────────────────────────────────────────────

  function processFiles(files) {
    Array.from(files).forEach(function (f) {
      if (f.name.endsWith(".md")) {
        chosenMd = f;
      } else if (/\.(jpe?g|png|gif|webp|svg|avif)$/i.test(f.name)) {
        if (
          !chosenImages.find(function (i) {
            return i.file.name === f.name;
          })
        ) {
          chosenImages.push({ file: f, origName: f.name });
        }
      }
    });

    setStatus("");
    submitBtn.disabled = !chosenMd;
    renderFileList();

    if (!chosenMd && files.length) {
      setStatus("Please include a .md file.", "error");
    }
  }

  fileInput.addEventListener("change", function () {
    processFiles(fileInput.files);
  });

  dropzone.addEventListener("dragover", function (e) {
    e.preventDefault();
    dropzone.classList.add("drag-over");
  });
  dropzone.addEventListener("dragleave", function () {
    dropzone.classList.remove("drag-over");
  });
  dropzone.addEventListener("drop", function (e) {
    e.preventDefault();
    dropzone.classList.remove("drag-over");
    processFiles(e.dataTransfer.files);
  });

  // ── Slug derivation ─────────────────────────────────────────────────────

  var CYRILLIC_MAP = {
    а: "a",
    б: "b",
    в: "v",
    г: "g",
    д: "d",
    е: "e",
    ё: "yo",
    ж: "zh",
    з: "z",
    и: "i",
    й: "y",
    к: "k",
    л: "l",
    м: "m",
    н: "n",
    о: "o",
    п: "p",
    р: "r",
    с: "s",
    т: "t",
    у: "u",
    ф: "f",
    х: "kh",
    ц: "ts",
    ч: "ch",
    ш: "sh",
    щ: "sch",
    ъ: "",
    ы: "y",
    ь: "",
    э: "e",
    ю: "yu",
    я: "ya",
  };

  function slugify(text) {
    var t = text
      .toLowerCase()
      .split("")
      .map(function (c) {
        return CYRILLIC_MAP.hasOwnProperty(c) ? CYRILLIC_MAP[c] : c;
      })
      .join("");
    var slug = t
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "")
      .replace(/-{2,}/g, "-");
    return slug || null;
  }

  function deriveSlug(mdText) {
    // 1. Front-matter slug field
    var fmMatch = mdText.match(/^\+{3}\n([\s\S]*?)\n\+{3}/);
    if (fmMatch) {
      var slugLine = fmMatch[1].match(/^\s*slug\s*=\s*"?([^"\n]+)"?/m);
      if (slugLine) {
        var s = slugLine[1].trim();
        if (s) return s;
      }
    }
    // 2. First H1 (search body only — skip past front matter if present)
    var body = fmMatch ? mdText.slice(mdText.indexOf("\n+++", 3) + 4) : mdText;
    var h1 = body.match(/^#\s+(.+)$/m);
    if (h1) {
      var s = slugify(h1[1].trim());
      if (s) return s;
    }
    // 3. Timestamp fallback
    return "article-" + Date.now();
  }
  // ── Image rewriting ─────────────────────────────────────────────────────

  function rewriteImageRefs(mdText, slug, images) {
    if (!images.length) return { rewritten: mdText, mapping: [] };

    var origNames = images.map(function (i) {
      return i.file.name;
    });
    var mapping = [];
    var counter = 1;

    var rewritten = mdText.replace(
      /!\[([^\]]*)\]\(([^)]+)\)(\{[^}]*\})?/g,
      function (match, alt, src, attrs) {
        var basename = src.split("/").pop().split("?")[0];
        var idx = origNames.indexOf(basename);
        if (idx === -1) return match;

        var ext = basename.split(".").pop().toLowerCase();
        var newName = slug + "-" + counter + "." + ext;
        mapping.push({
          origName: basename,
          newName: newName,
          index: counter,
          file: images[idx].file,
        });
        counter++;
        return "![" + alt + "](/static/img/" + newName + ")" + (attrs || "");
      },
    );

    // Second pass: Obsidian wiki-link images  ![[path/to/image.png]]
    rewritten = rewritten.replace(
      /!\[\[([^\]]+)\]\]/g,
      function (match, src) {
        var basename = src.split("/").pop().split("?")[0];
        var idx = origNames.indexOf(basename);
        if (idx === -1) return match;

        var ext = basename.split(".").pop().toLowerCase();
        var newName = slug + "-" + counter + "." + ext;
        mapping.push({
          origName: basename,
          newName: newName,
          index: counter,
          file: images[idx].file,
        });
        counter++;
        return "![](/static/img/" + newName + ")";
      },
    );

    return { rewritten: rewritten, mapping: mapping };
  }

  // ── Submit flow ─────────────────────────────────────────────────────────

  submitBtn.addEventListener("click", function () {
    if (!chosenMd) return;
    submitBtn.disabled = true;
    setStatus("Reading article…", "info");

    var reader = new FileReader();
    reader.onload = function (e) {
      var mdText = e.target.result;
      var slug = deriveSlug(mdText);
      var result = rewriteImageRefs(mdText, slug, chosenImages);
      var rewrittenMd = result.rewritten;
      var mapping = result.mapping;

      if (!mapping.length) {
        publishArticle(rewrittenMd);
        return;
      }

      setStatus("Uploading images (0/" + mapping.length + ")…", "info");
      uploadImages(mapping, slug, 0, function (err) {
        if (err) {
          setStatus("Image upload failed: " + err, "error");
          submitBtn.disabled = false;
          return;
        }
        publishArticle(rewrittenMd);
      });
    };
    reader.readAsText(chosenMd);
  });

  function uploadImages(mapping, slug, idx, done) {
    if (idx >= mapping.length) {
      done(null);
      return;
    }
    var item = mapping[idx];
    setStatus(
      "Uploading images (" + (idx + 1) + "/" + mapping.length + ")…",
      "info",
    );
    var fr = new FileReader();
    fr.onload = function (ev) {
      var buf = ev.target.result;
      fetch("/admin/image", {
        method: "POST",
        headers: {
          "Content-Type": "application/octet-stream",
          "X-Image-Name": item.origName,
          "X-Article-Slug": slug,
          "X-Image-Index": String(item.index),
        },
        body: buf,
        credentials: "include",
      })
        .then(function (r) {
          if (!r.ok)
            return r.text().then(function (t) {
              throw new Error(t);
            });
          uploadImages(mapping, slug, idx + 1, done);
        })
        .catch(function (e) {
          done(e.message);
        });
    };
    fr.readAsArrayBuffer(item.file);
  }

  function publishArticle(mdText) {
    setStatus("Publishing…", "info");
    fetch("/admin/article", {
      method: "POST",
      headers: { "Content-Type": "text/plain" },
      body: mdText,
      credentials: "include",
    })
      .then(function (r) {
        if (r.ok)
          return r.text().then(function (slug) {
            setStatus("Published! Redirecting…", "success");
            setTimeout(function () {
              window.location = "/article/" + slug;
            }, 800);
          });
        return r.text().then(function (t) {
          throw new Error(t);
        });
      })
      .catch(function (e) {
        setStatus(e.message, "error");
        submitBtn.disabled = false;
      });
  }
})();
