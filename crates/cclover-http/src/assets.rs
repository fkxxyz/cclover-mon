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
events.onmessage = event => { root.innerHTML = JSON.parse(event.data); };
"#;
