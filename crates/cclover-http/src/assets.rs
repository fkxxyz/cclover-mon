pub(super) const INDEX_HTML: &str = r#"<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>cclover-mon</title>
  <link rel="stylesheet" href="/style.css">
</head>
<body>
  <div id="cclover-root"></div>
  <script src="/bootstrap.js"></script>
</body>
</html>
"#;

pub(super) const BOOTSTRAP_JS: &str = r#"const root = document.getElementById('cclover-root');
const events = new EventSource('/events');
let typographyFailureReported = false;

function typographyFits(panel) {
  const tolerance = 1 / (window.devicePixelRatio || 1);
  for (const text of panel.querySelectorAll('text[data-cclover-must-fit]')) {
    const maxWidth = Number(text.dataset.ccloverMaxWidth);
    if (!Number.isFinite(maxWidth) || text.getComputedTextLength() > maxWidth + tolerance) return false;
  }
  return true;
}

function installDashboard(markup) {
  const previous = root.innerHTML;
  root.innerHTML = markup;
  const panel = root.querySelector('.cclover-panel');
  if (!panel || !typographyFits(panel)) {
    root.innerHTML = previous;
    if (!typographyFailureReported) {
      console.error('cclover-mon: graphical text exceeded its authoritative Scene slot');
      typographyFailureReported = true;
    }
    return false;
  }
  typographyFailureReported = false;
  return true;
}

events.onmessage = event => { installDashboard(JSON.parse(event.data)); };
"#;
