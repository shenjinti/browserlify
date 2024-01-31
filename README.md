Browserlify  - browser as a service
=======

Browserlify is a browser service:
- Content API: Retrieve web page content by URL, supporting PDF generation, screenshot capture, text extraction, and HTML dumping.
- Headless Chrome: Connect to Headless Chrome via WebSocket and perform testing using Puppeteer.
- Remote Browser: Implement a remote browser based on noVNC, allowing browser access through a web interface.

With Browserlify, you can easily access web content, automate testing with Headless Chrome, and enjoy the convenience of a remote browser.

### How to run (via docker)
```bash
# WIP: docker image is not ready yet
#docker run  -ti --rm --privileged -p 9000:9000 shenjinti/browserlify
```

### Run via cargo & puppeteer (local development)
> Note: you need to install puppeteer and rust first
> Only tested on linux

```bash
$ cd browserlify
$ cargo run
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
- The parameters:
```javascript
{
  "url": "https://example.org",

  "file_name": "example.pdf",
  "timeout": 60000, // total timeout: milliseconds
  "wait_load": 1000, // wait for load: milliseconds
  "selector": "#main", // wait for selector: css selector
  "images": true, // wait for images loaded
  "network_idle": 1000, // wait for network idle: milliseconds
  "page_ready": true,
  "scroll_bottom": 1000, // milliseconds
  "scroll_interval": 1000, // milliseconds

  "paper_width": 8.5, //pdf: inches
  "paper_height": 11, //pdf: inches
  "scale": 1,
  "margin_top": 0.4, // inches
  "margin_bottom": 0.4, // inches
  "margin_left": 0.4, // inches
  "margin_right": 0.4, // inches
  "background": true, // pdf: print background
  "landscape": false, // landscape or portrait
  "page_ranges": "1-5",  // pdf: page ranges: 1-5, 1,2,3
  "device": "ipad",  // emulate device: iphone, ipad, 2k, 4k
  "disable_link": true,  // pdf: disable link
  "paper_size": "A4",  // pdf: paper size: A4, A3, Letter, Legal
  "header_template": "<div>Header</div>",
  "footer_template": "<div>Footer</div>",
  "format": "png",    // screenshot: format: png, jpeg, pdf
  "quality": 100,     // screenshot: only used for jpeg format
  "clip": "0,0,800,600",   // screenshot: clip the screenshot to the specified rectangle
  "full_page": true,       // screenshot: capture the full scrollable page, not just the viewport
  "author": "Browserlify", // pdf: author
}
```

- `/pdf` - generate pdf from url
```
curl "http://localhost:9000/pdf?url=http://browserlify.com&images=true" > browserlify.pdf
```
- `/screenshot` - generate screenshot from url
```
curl "http://localhost:9000/screenshot?url=http://browserlify.com&format=png&full_page=true" > browserlify.png
```
- `/text` - dump dom text from url
```
curl "http://localhost:9000/text?url=http://browserlify.com" > browserlify.txt
```
- `/html` - dump html content from url
```
curl "http://localhost:9000/html?url=http://browserlify.com" > browserlify.html
```

