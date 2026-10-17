// `prepublishOnly` guard: refuses to publish unless dist/ holds a
// complete build, nothing else, and no source changed after it. It
// only inspects the files — it never builds — so publishing stays a
// separate, explicit step after `npm run build`.
import { checkBuildOutput } from './package-files.mjs';

const { missing, unexpected, stale } = checkBuildOutput();

if (missing.length > 0 || unexpected.length > 0 || stale.length > 0) {
  for (const path of missing) console.error(`missing:    ${path}`);
  for (const path of unexpected) console.error(`unexpected: ${path}`);
  for (const path of stale) console.error(`changed after the build: ${path}`);
  console.error(
    'logic-mesh: the build output is incomplete or out of date; run `npm run build` before publishing.',
  );
  process.exit(1);
}
