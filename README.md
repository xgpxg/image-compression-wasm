## image-compression-wasm

An image compression tool for WASM, supporting PNG, JPG, WEBP, and GIF formats.

## Building


```shell
wasm-pack build --target web
```

This will generate a pkg folder that can be imported into frontend projects as JS/TS files.

## Usage in JavaScript

Since image compression mainly involves CPU-intensive computations, use Web Workers for asynchronous execution to prevent UI blocking. Below is the Worker implementation:

```javascript
// workers/compression-image.js
import init, {compress} from "image-compression-wasm";

init().then(() => {
    self.onmessage = function (event) {
        const {bytes, quality, resizePercent} = event.data;

        const compressedImageBytes = compress(bytes, quality, resizePercent);

        self.postMessage(compressedImageBytes,);
    };
})
```

Import and use the Worker like this:

```javascript
const worker = new Worker(new URL('workers/compression-image.js', import.meta.url), {type: 'module'});

// Note: This is the file object selected from the file selection box. Please replace your selection logic
const file = null;
// Compression quality, range 0-100
const quality = 50;
// Compressed size, range 0-100
const resizePercent = 100;

const reader = new FileReader();
reader.readAsArrayBuffer(file);
reader.onload = (event) => {
    const result = event.target?.result;
    const uint8Array = new Uint8Array(result);

    // Execute compression
    worker.postMessage({
        bytes: uint8Array,
        quality,
        resizePercent: resizePercent / 100,
    });

    // Compression completed
    worker.onmessage = async (event) => {
        // Compressed image data
        const bytes = event.data;
        // You can create a Blob object to display or download images
        const blobUrl = URL.createObjectURL(new Blob([bytes], {type: file.type}));
        // Your other code
        // ...
    }

}

```