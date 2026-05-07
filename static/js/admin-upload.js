(function () {
    'use strict';

    var panel = document.getElementById('admin-panel');
    if (!panel) return; // not an admin session, nothing to do

    var dropzone = document.getElementById('admin-dropzone');
    var fileInput = document.getElementById('admin-file');
    var status = document.getElementById('admin-status');
    var submitBtn = document.getElementById('admin-submit');
    var closeBtn = document.getElementById('admin-close');
    var trigger = document.getElementById('upload-trigger');

    // { name: File } for images keyed by original filename
    var chosenMd = null;
    var chosenImages = []; // [{file, origName}]

    if (trigger) {
        trigger.addEventListener('click', function (e) {
            e.preventDefault();
            resetPanel();
            panel.style.display = 'flex';
        });
    }

    closeBtn.addEventListener('click', function () { panel.style.display = 'none'; });
    panel.addEventListener('click', function (e) { if (e.target === panel) panel.style.display = 'none'; });

    function resetPanel() {
        status.textContent = '';
        fileInput.value = '';
        chosenMd = null;
        chosenImages = [];
        submitBtn.disabled = true;
        renderFileList();
    }

    function renderFileList() {
        var list = document.getElementById('admin-file-list');
        if (!list) return;
        list.innerHTML = '';
        if (chosenMd) {
            var li = document.createElement('li');
            li.textContent = '📄 ' + chosenMd.name;
            list.appendChild(li);
        }
        chosenImages.forEach(function (img) {
            var li = document.createElement('li');
            li.textContent = '🖼 ' + img.file.name;
            list.appendChild(li);
        });
    }

    function processFiles(files) {
        Array.from(files).forEach(function (f) {
            if (f.name.endsWith('.md')) {
                chosenMd = f;
            } else if (/\.(jpe?g|png|gif|webp|svg|avif)$/i.test(f.name)) {
                // deduplicate by name
                if (!chosenImages.find(function (i) { return i.file.name === f.name; })) {
                    chosenImages.push({ file: f, origName: f.name });
                }
            }
        });
        status.textContent = '';
        submitBtn.disabled = !chosenMd;
        renderFileList();
        if (!chosenMd && files.length) {
            status.textContent = 'Please include a .md file.';
        }
    }

    fileInput.addEventListener('change', function () { processFiles(fileInput.files); });
    dropzone.addEventListener('dragover', function (e) { e.preventDefault(); dropzone.classList.add('drag-over'); });
    dropzone.addEventListener('dragleave', function () { dropzone.classList.remove('drag-over'); });
    dropzone.addEventListener('drop', function (e) {
        e.preventDefault();
        dropzone.classList.remove('drag-over');
        processFiles(e.dataTransfer.files);
    });

    /* -- Slug derivation (mirrors server logic) -- */
    function slugify(text) {
        return text.toLowerCase()
            .replace(/[^a-z0-9]+/g, '-')
            .replace(/^-+|-+$/g, '')
            .replace(/-{2,}/g, '-');
    }

    function deriveSlug(mdText) {
        // front-matter slug field
        var fmMatch = mdText.match(/^\+{3}\n([\s\S]*?)\n\+{3}/);
        if (fmMatch) {
            var slugLine = fmMatch[1].match(/^\s*slug\s*=\s*"?([^"\n]+)"?/m);
            if (slugLine) return slugLine[1].trim();
        }
        // first H1
        var h1 = mdText.match(/^#\s+(.+)$/m);
        if (h1) return slugify(h1[1].trim());
        return 'article-' + Date.now();
    }

    /* -- Rewrite image references in markdown -- */
    function rewriteImageRefs(mdText, slug, images) {
        if (!images.length) return { rewritten: mdText, mapping: [] };

        var origNames = images.map(function (i) { return i.file.name; });
        var mapping = [];
        var counter = 1;

        var rewritten = mdText.replace(
            /!\[([^\]]*)\]\(([^)]+)\)(\{[^}]*\})?/g,
            function (match, alt, src, attrs) {
                var basename = src.split('/').pop().split('?')[0];
                var idx = origNames.indexOf(basename);
                if (idx === -1) return match;

                var ext = basename.split('.').pop().toLowerCase();
                var newName = slug + '-' + counter + '.' + ext;
                mapping.push({ origName: basename, newName: newName, index: counter, file: images[idx].file });
                counter++;
                return '![' + alt + '](/static/img/' + newName + ')' + (attrs || '');
            }
        );

        return { rewritten: rewritten, mapping: mapping };
    }

    /* -- Submit -- */
    submitBtn.addEventListener('click', function () {
        if (!chosenMd) return;
        submitBtn.disabled = true;
        status.textContent = 'Reading article…';

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

            status.textContent = 'Uploading images (0/' + mapping.length + ')…';
            uploadImages(mapping, slug, 0, function (err) {
                if (err) {
                    status.textContent = '✗ Image upload failed: ' + err;
                    submitBtn.disabled = false;
                    return;
                }
                publishArticle(rewrittenMd);
            });
        };
        reader.readAsText(chosenMd);
    });

    function uploadImages(mapping, slug, idx, done) {
        if (idx >= mapping.length) { done(null); return; }
        var item = mapping[idx];
        status.textContent = 'Uploading images (' + (idx + 1) + '/' + mapping.length + ')…';
        var fr = new FileReader();
        fr.onload = function (ev) {
            var buf = ev.target.result;
            fetch('/admin/image', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/octet-stream',
                    'X-Image-Name': item.origName,
                    'X-Article-Slug': slug,
                    'X-Image-Index': String(item.index),
                },
                body: buf,
                credentials: 'include',
            }).then(function (r) {
                if (!r.ok) return r.text().then(function (t) { throw new Error(t); });
                uploadImages(mapping, slug, idx + 1, done);
            }).catch(function (e) { done(e.message); });
        };
        fr.readAsArrayBuffer(item.file);
    }

    function publishArticle(mdText) {
        status.textContent = 'Publishing…';
        fetch('/admin/article', {
            method: 'POST',
            headers: { 'Content-Type': 'text/plain' },
            body: mdText,
            credentials: 'include',
        })
            .then(function (r) {
                if (r.ok) return r.text().then(function (slug) {
                    status.textContent = '✓ Published! Redirecting…';
                    setTimeout(function () { window.location = '/article/' + slug; }, 800);
                });
                return r.text().then(function (t) { throw new Error(t); });
            })
            .catch(function (e) {
                status.textContent = '✗ ' + e.message;
                submitBtn.disabled = false;
            });
    }
})();