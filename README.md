Browserlify  - browser as a service
=======

### How to run (via docker)
```bash
docker run --cap-add=SYS_ADMIN -p 9000:9000 -ti --rm shenjinti/browserlify
```

### Run via cargo & puppeteer
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
```
curl "http://localhost:9000/pdf?url=http://browserlify.com" > browserlify.pdf
```
- `/screenshot` - generate screenshot from url
```
curl "http://localhost:9000/screenshot?url=http://browserlify.com" > browserlify.png
```
- `/text` - dump dom text from url
```
curl "http://localhost:9000/text?url=http://browserlify.com" > browserlify.txt
```
- `/html` - dump html content from url
```
curl "http://localhost:9000/html?url=http://browserlify.com" > browserlify.html
```

