// Copies three.js add-ons (examples/jsm) into vendor/ so they can be packaged
// (electron-builder always excludes node_modules/*/examples).
const fs = require('fs'); const path = require('path');
const src = path.join(__dirname, '..', 'node_modules', 'three', 'examples', 'jsm');
const dst = path.join(__dirname, '..', 'vendor', 'three-addons');
fs.rmSync(dst, { recursive: true, force: true });
fs.cpSync(src, dst, { recursive: true, filter: (p) => !p.endsWith('.d.ts') });
console.log('vendored three addons ->', dst);
