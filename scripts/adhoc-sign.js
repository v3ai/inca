// electron-builder afterPack hook: without an Apple Developer certificate the Mac app is left unsigned,
// and Apple Silicon Macs refuse to run unsigned code ("Inca is damaged"). An ad-hoc signature is free
// and enough to run it; users still confirm the first launch because it isn't notarized.
const { execFileSync } = require('child_process'); const path = require('path');
exports.default = async (ctx) => {
  if (ctx.electronPlatformName !== 'darwin') return;
  const app = path.join(ctx.appOutDir, ctx.packager.appInfo.productFilename + '.app');
  execFileSync('codesign', ['--force', '--deep', '--sign', '-', app], { stdio: 'inherit' });
  execFileSync('codesign', ['--verify', '--deep', '--strict', app], { stdio: 'inherit' });
  console.log('ad-hoc signed', app);
};
