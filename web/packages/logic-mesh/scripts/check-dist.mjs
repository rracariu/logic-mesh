// `prepublishOnly` guard: refuses to publish unless dist/ holds a
// complete build and nothing else. It only inspects the files — it
// never builds — so publishing stays a separate, explicit step after
// `npm run build`.
import { checkBuildOutput } from './package-files.mjs';

const { missing, unexpected } = checkBuildOutput();

if (missing.length > 0 || unexpected.length > 0) {
  for (const path of missing) console.error(`missing:    ${path}`);
  for (const path of unexpected) console.error(`unexpected: ${path}`);
  console.error(
    'logic-mesh: the build output is incomplete or stale; run `npm run build` before publishing.',
  );
  process.exit(1);
}
