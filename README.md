Browserlify  - browser as a service
=======

### How to run
```bash
$ cd browserlify
$ cargo run --
```
Test via puppeteer
```javascript
  const browser = await puppeteer.connect({
    browserWSEndpoint: `ws://localhost:9000`,
  });
  const page = await browser.newPage();
  await page.goto('https://example.org');
  await page.screenshot({ path: 'example.png' });
```

### API
- `/list` - list all session
- `/kill/:session_id` - kill session by id
- `/kill_all` - kill all sessions
