(function () {
  var NAV_ITEMS = [
    { group: 'Start', items: [
      { href: 'index.html', label: 'Getting Started' },
    ] },
    { group: 'Reference', items: [
      { href: 'config.html', label: 'Configuration' },
    ] },
  ];

  var EXT_ITEMS = [
    { href: 'https://github.com/shortcuts/clidraw/blob/main/CONTRIBUTING.md', label: 'Contributing' },
    { href: 'https://github.com/shortcuts/clidraw/issues/new?labels=enhancement', label: 'Request a feature' },
    { href: 'https://github.com/shortcuts/clidraw/issues/new?labels=bug', label: 'Report a bug' },
  ];

  var currentFile = location.pathname.split('/').pop() || 'index.html';

  var navHtml = NAV_ITEMS.map(function (g) {
    var gid = 'nav-group-' + g.group.toLowerCase();
    return '<div role="group" aria-labelledby="' + gid + '"><span class="nav-group" id="' + gid + '">' + g.group + '</span>' + g.items.map(function (item) {
      var cls = item.href === currentFile ? ' class="active" aria-current="page"' : '';
      return '<a href="' + item.href + '"' + cls + '>' + item.label + '</a>';
    }).join('') + '</div>';
  }).join('');

  navHtml += '<span class="nav-sep"></span>';
  navHtml += EXT_ITEMS.map(function (item) {
    return '<a class="nav-ext" href="' + item.href + '" target="_blank" rel="noopener">' + item.label + '</a>';
  }).join('');

  var aside = '<aside class="sidebar">'
    + '<a class="brand" href="index.html">'
    +   '<span class="brand-name">clidraw</span>'
    + '</a>'
    + '<button class="nav-toggle-btn" aria-label="Toggle navigation" aria-expanded="false" aria-controls="site-nav">'
    +   '<svg width="16" height="16" viewBox="0 0 16 16" fill="none">'
    +     '<path d="M2 4h12M2 8h12M2 12h12" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>'
    +   '</svg>'
    + '</button>'
    + '<nav id="site-nav" aria-label="Site">' + navHtml + '</nav>'
    + '<div class="sidebar-foot">'
    +   '<a class="gh-star" href="https://github.com/shortcuts/clidraw" target="_blank" rel="noopener">'
    +     '<svg width="13" height="13" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">'
    +       '<path d="M8 .25a.75.75 0 0 1 .673.418l1.882 3.815 4.21.612a.75.75 0 0 1 .416 1.279'
    +         'l-3.046 2.97.719 4.192a.751.751 0 0 1-1.088.791L8 12.347l-3.766 1.98a.75.75 0 0 1'
    +         '-1.088-.79l.72-4.194L.872 6.374a.75.75 0 0 1 .416-1.28l4.21-.611L7.327.668A.75.75 0 0 1 8 .25Z"/>'
    +     '</svg>'
    +     'Star'
    +     '<span class="gh-star-sep"></span>'
    +     '<span class="gh-star-count">—</span>'
    +   '</a><a class="sidebar-foot-lic" href="https://github.com/sponsors/shortcuts" target="_blank" rel="noopener">Sponsor</a>'
    +   '<span class="sidebar-foot-lic">· MIT License</span>'
    + '</div>'
    + '</aside>';

  document.querySelector('.layout').insertAdjacentHTML('afterbegin', aside);

  var mainEl = document.querySelector('main');
  if (!mainEl.id) mainEl.id = 'main-content';
  document.body.insertAdjacentHTML('afterbegin', '<a class="skip-link" href="#' + mainEl.id + '">Skip to content</a>'
    + '<div class="sr-only" id="live-status" role="status"></div>');

  var toggle = document.querySelector('.nav-toggle-btn');
  toggle.addEventListener('click', function () {
    toggle.setAttribute('aria-expanded', String(document.getElementById('site-nav').classList.toggle('open')));
  });

  // Unauthenticated GitHub API allows 60 requests/hour per IP; cache the count for the tab session.
  var showStars = function (t) {
    document.querySelectorAll('.gh-star-count').forEach(function (el) { el.textContent = t; });
  };
  var cachedStars = null;
  try { cachedStars = sessionStorage.getItem('clidraw-stars'); } catch (e) {}
  if (cachedStars) {
    showStars(cachedStars);
  } else {
    fetch('https://api.github.com/repos/shortcuts/clidraw')
      .then(function (r) { return r.json(); })
      .then(function (d) {
        var n = d.stargazers_count;
        var t = n >= 1000 ? (n / 1000).toFixed(1) + 'k' : String(n);
        showStars(t);
        try { sessionStorage.setItem('clidraw-stars', t); } catch (e) {}
      })
      .catch(function () {});
  }

  var pageLink = document.querySelector('nav a.active');
  var outlineHeads = document.querySelectorAll('main h2[id]:not(.item), main h3[id]:not(.item)');
  if (pageLink && outlineHeads.length) {
    var outline = document.createElement('div');
    outline.className = 'nav-outline';
    var outlineLinks = [];
    outlineHeads.forEach(function (h) {
      var a = document.createElement('a');
      a.className = 'nav-' + h.tagName.toLowerCase();
      a.href = '#' + h.id;
      a.textContent = h.textContent.trim();
      outline.appendChild(a);
      outlineLinks.push(a);
    });
    pageLink.after(outline);

    var activeLink = null;
    // Near page end the target heading cannot reach the top, so later headings also pass the line; pin the hash target until the user scrolls.
    var pinned = -1;
    var pinHash = function () {
      var id = decodeURIComponent(location.hash.slice(1));
      pinned = Array.prototype.findIndex.call(outlineHeads, function (h) { return h.id === id; });
    };
    var spy = function () {
      var idx = 0;
      outlineHeads.forEach(function (h, i) {
        if (h.getBoundingClientRect().top <= window.innerHeight * 0.3) idx = i;
      });
      if (pinned >= 0) idx = pinned;
      var next = outlineLinks[idx];
      if (next === activeLink) return;
      if (activeLink) {
        activeLink.classList.remove('active');
        activeLink.removeAttribute('aria-current');
      }
      next.classList.add('active');
      next.setAttribute('aria-current', 'true');
      activeLink = next;
      if (window.innerWidth > 680) next.scrollIntoView({ block: 'nearest' });
    };
    var io = new IntersectionObserver(spy, { rootMargin: '0px 0px -70% 0px' });
    outlineHeads.forEach(function (h) { io.observe(h); });
    window.addEventListener('hashchange', function () { pinHash(); spy(); });
    ['wheel', 'touchmove', 'keydown'].forEach(function (e) {
      window.addEventListener(e, function () { pinned = -1; }, { passive: true });
    });
    pinHash();
    spy();
  }

  // "On this page" list: the sidebar outline is hidden behind the menu button on phones, so CSS shows this one only there.
  var tocHeads = document.querySelectorAll('main h2[id]:not(.item)');
  var lead = mainEl.querySelector('h1 + p') || mainEl.querySelector('h1');
  if (lead && tocHeads.length >= 2) {
    var toc = document.createElement('nav');
    toc.className = 'page-toc';
    toc.setAttribute('aria-label', 'On this page');
    toc.innerHTML = '<span class="page-toc-title">On this page</span>';
    tocHeads.forEach(function (h) {
      var a = document.createElement('a');
      a.href = '#' + h.id;
      a.textContent = h.textContent.trim();
      toc.appendChild(a);
    });
    lead.after(toc);
  }

  document.querySelectorAll('main h1[id], main h2[id], main h3[id]').forEach(function (heading) {
    var btn = document.createElement('button');
    btn.className = 'heading-link';
    btn.type = 'button';
    btn.setAttribute('aria-label', 'Copy link to this section');
    btn.innerHTML = '<svg width="13" height="13" viewBox="0 0 16 16" fill="none">'
      + '<path d="M6.5 9.5a3.5 3.5 0 0 0 5 0l2-2a3.5 3.5 0 0 0-5-5l-1 1" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>'
      + '<path d="M9.5 6.5a3.5 3.5 0 0 0-5 0l-2 2a3.5 3.5 0 0 0 5 5l1-1" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>'
      + '</svg>';
    btn.addEventListener('click', function () {
      var url = location.origin + location.pathname + '#' + heading.id;
      var done = function () {
        btn.classList.add('copied');
        document.getElementById('live-status').textContent = 'Link copied';
        setTimeout(function () {
          btn.classList.remove('copied');
          document.getElementById('live-status').textContent = '';
        }, 1200);
      };
      // clipboard is undefined on non-secure origins; fall back to putting the link in the address bar.
      if (navigator.clipboard) navigator.clipboard.writeText(url).then(done, function () {});
      else location.hash = heading.id;
    });
    heading.appendChild(btn);
  });
}());
