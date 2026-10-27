(function () {
  var errorEl = document.getElementById('error');
  var copyBtn = document.getElementById('copy-source');
  var downloadBtn = document.getElementById('download-svg');

  var fail = function (msg) {
    errorEl.textContent = msg;
    errorEl.hidden = false;
  };

  var decode = function (fragment) {
    var b64 = fragment.replace(/-/g, '+').replace(/_/g, '/');
    while (b64.length % 4) b64 += '=';
    var bin = atob(b64);
    var bytes = Uint8Array.from(bin, function (c) { return c.charCodeAt(0); });
    return new TextDecoder('utf-8', { fatal: true }).decode(bytes);
  };

  var sanitize = function (svg) {
    var doc = new DOMParser().parseFromString(svg, 'image/svg+xml');
    doc.querySelectorAll('script, foreignObject').forEach(function (el) { el.remove(); });
    doc.querySelectorAll('*').forEach(function (el) {
      Array.prototype.slice.call(el.attributes).forEach(function (attr) {
        var name = attr.name.toLowerCase();
        var isHref = name === 'href' || name === 'xlink:href';
        if (name.indexOf('on') === 0 || (isHref && attr.value.indexOf('#') !== 0)) {
          el.removeAttribute(attr.name);
        }
      });
    });
    return document.importNode(doc.documentElement, true);
  };

  window.addEventListener('hashchange', function () {
    if (location.hash !== '#main-content') location.reload();
  });

  var fragment = location.hash.slice(1);
  if (!fragment || fragment === 'main-content') return fail('This link has no diagram.');
  if (!window.pele) return fail('The renderer failed to load.');

  var text;
  try {
    text = decode(fragment);
  } catch (e) {
    return fail('This link is damaged and cannot be decoded.');
  }

  var mounted = window.pele.mount(document.getElementById('diagram'), text, {
    zoom: true,
    links: false,
    images: false,
    sanitize: sanitize,
    onError: function (e) { fail(e && e.message ? e.message : String(e)); },
  });
  if (!mounted.result) return;

  copyBtn.disabled = false;
  downloadBtn.disabled = false;

  copyBtn.addEventListener('click', function () {
    if (!navigator.clipboard) return;
    navigator.clipboard.writeText(text).then(function () {
      document.getElementById('live-status').textContent = 'Copied';
      setTimeout(function () { document.getElementById('live-status').textContent = ''; }, 1200);
    }, function () {});
  });

  downloadBtn.addEventListener('click', function () {
    var svg = new XMLSerializer().serializeToString(sanitize(mounted.result.svg));
    var a = document.createElement('a');
    a.href = URL.createObjectURL(new Blob([svg], { type: 'image/svg+xml' }));
    a.download = 'diagram.svg';
    a.click();
    URL.revokeObjectURL(a.href);
  });
}());
