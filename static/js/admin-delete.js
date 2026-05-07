(function () {
    'use strict';

    var toast = document.getElementById('toast');
    var toastTimer;

    function showToast(msg, isError) {
        clearTimeout(toastTimer);
        toast.textContent = msg;
        toast.className = 'toast' + (isError ? ' error' : '') + ' show';
        toastTimer = setTimeout(function () { toast.className = 'toast'; }, 2800);
    }

    var articleList = document.getElementById('article-list');
    if (!articleList) return;

    articleList.addEventListener('click', function (e) {
        var btn = e.target.closest('.delete-btn');
        if (!btn) return;

        var slug = btn.dataset.slug;
        var entry = document.getElementById('entry-' + slug);
        var title = entry.querySelector('.article-entry__title')?.textContent?.trim() || slug;

        if (!confirm('Delete "' + title + '"?\nThis cannot be undone.')) return;

        btn.disabled = true;
        btn.textContent = 'Deleting…';

        fetch('/admin/article/' + encodeURIComponent(slug), {
            method: 'DELETE',
            credentials: 'include',
        })
            .then(function (r) {
                if (r.status === 204) {
                    entry.style.transition = 'opacity 0.25s';
                    entry.style.opacity = '0';
                    setTimeout(function () { entry.remove(); }, 260);
                    showToast('Deleted "' + title + '"', false);
                } else {
                    return r.text().then(function (t) { throw new Error(t || r.status); });
                }
            })
            .catch(function (err) {
                btn.disabled = false;
                btn.textContent = 'Delete';
                showToast('Error: ' + err.message, true);
            });
    });
})();