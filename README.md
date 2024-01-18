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

### Session API
- `/list` - list all session
- `/kill/:session_id` - kill session by id
- `/kill_all` - kill all sessions
### Content API
- `/pdf` - generate pdf from url
- `/screenshot` - generate screenshot from url
- `/text` - dump dom text from url
- `/html` - dump html content from url
