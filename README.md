Chrome server
=======

### How to run
```bash
$ cd chrome_server
$ cargo run
```
Test via puppeteer
```javascript
  const browser = await puppeteer.connect({
    browserWSEndpoint: `ws://localhost:8080`,
  });
  const page = await browser.newPage();
  await page.goto('https://example.org');
  await page.screenshot({ path: 'example.png' });
```

### API
- `/api/list` - list all session
- `/api/kill/:session_id` - kill session by id
- `/api/kill_all` - kill all sessions
