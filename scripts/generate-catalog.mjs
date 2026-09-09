import { readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';

const manifest = JSON.parse(await readFile(new URL('../src-tauri/seed/catalog.json', import.meta.url), 'utf8'));
const union = (kind) => manifest.courses.filter((course) => course.kind === kind).map((course) => JSON.stringify(course.id)).join(' | ');
const generated = `// Generated from src-tauri/seed/catalog.json. Run npm run catalog:generate.\nexport type FocusArea = ${union('engineering')};\nexport type LanguageId = ${union('language')};\nexport type ClassroomSubjectId = FocusArea | LanguageId;\n`;
const destination = new URL('../src/lib/catalog.generated.ts', import.meta.url);
if (process.argv.includes('--check')) {
  if (await readFile(destination, 'utf8').catch(() => '') !== generated) {
    throw new Error(`Catalog types are stale: run npm run catalog:generate (${fileURLToPath(destination)})`);
  }
} else {
  await writeFile(destination, generated);
}
