const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const vm = require('node:vm');
const ts = require('typescript');

const root = process.env.FLUXVOICE_TEST_ROOT || path.resolve(__dirname, '..');

function harness() {
  const slots = [];
  const calls = [];
  const errors = [];
  let cursor = 0;
  let resolve;
  let reject;
  let clears = 0;
  const gate = new Promise((yes, no) => { resolve = yes; reject = no; });
  const react = {
    useRef(value) {
      const index = cursor++;
      return slots[index] ||= { current: value };
    },
    useState(value) {
      const index = cursor++;
      slots[index] ||= { value };
      return [slots[index].value, next => { slots[index].value = next; }];
    },
  };
  const jsx = (type, props) => ({ type, props });
  const dependencies = {
    react,
    'react/jsx-runtime': { jsx, jsxs: jsx },
    'lucide-react': {},
    '@tauri-apps/plugin-dialog': { save: async () => null },
    '@tauri-apps/plugin-fs': { writeFile: async () => {} },
    '@tauri-apps/api/core': { invoke: (...args) => { calls.push(args); return gate; } },
    '../../hooks/useTranscriptionHistory': {
      useTranscriptionHistory: () => ({
        transcriptionHistory: [{ timestamp: 1, original: 'one', finalText: 'one' }],
        clearHistory: () => { clears++; },
      }),
    },
    '../../utils/audioStorage': {},
  };
  const filename = path.join(root, 'src', 'components', 'ConfigPage', 'TranscriptionHistory.tsx');
  const compiled = ts.transpileModule(fs.readFileSync(filename, 'utf8'), {
    compilerOptions: {
      target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
      jsx: ts.JsxEmit.ReactJSX, esModuleInterop: true,
    },
    fileName: filename,
  }).outputText;
  const module = { exports: {} };
  vm.runInNewContext(compiled, {
    module, exports: module.exports, Date,
    require: name => {
      assert.ok(name in dependencies, `Unexpected dependency: ${name}`);
      return dependencies[name];
    },
    confirm: () => true,
    console: { error: (...args) => errors.push(args) },
  }, { filename });
  function render() {
    cursor = 0;
    return module.exports.TranscriptionHistory();
  }
  return { render, resolve, reject, calls, errors, clears: () => clears };
}

function nodes(tree) {
  if (!tree || typeof tree !== 'object') return [];
  if (Array.isArray(tree)) return tree.flatMap(nodes);
  return [tree, ...nodes(tree.props?.children)];
}

function text(tree) {
  if (tree == null || typeof tree === 'boolean') return '';
  if (typeof tree !== 'object') return String(tree);
  if (Array.isArray(tree)) return tree.map(text).join('');
  return text(tree.props?.children);
}

function button(tree, label) {
  return nodes(tree).find(node => node.type === 'button' && text(node).includes(label));
}

test('export invokes native command without audio IPC and blocks duplicate clicks and clear', async () => {
  const h = harness();
  const initial = h.render();
  const exporting = button(initial, 'Export All').props.onClick();
  await button(initial, 'Export All').props.onClick();
  button(initial, 'Clear All').props.onClick();
  assert.equal(h.calls.length, 1);
  assert.equal(h.calls[0].length, 1);
  assert.equal(h.calls[0][0], 'export_history');
  assert.equal(h.clears(), 0);
  const busy = h.render();
  assert.equal(button(busy, 'Exporting...').props.disabled, true);
  assert.equal(button(busy, 'Clear All').props.disabled, true);
  assert.ok(nodes(busy).some(node => node.props?.role === 'status'));
  h.resolve({ directory: 'C:\\Exports\\new', transcription_count: 3, audio_count: 2, missing_audio_count: 1 });
  await exporting;
  const done = h.render();
  assert.equal(button(done, 'Export All').props.disabled, false);
  assert.ok(text(done).includes('Exported 2 audio files and 3 transcripts.'));
  assert.ok(text(done).includes('C:\\Exports\\new'));
  assert.ok(text(done).includes('1 recordings had no saved audio'));
});

test('folder picker cancellation re-enables export without a success or error message', async () => {
  const h = harness();
  const exporting = button(h.render(), 'Export All').props.onClick();
  h.resolve(null);
  await exporting;
  const done = h.render();
  assert.equal(button(done, 'Export All').props.disabled, false);
  assert.equal(nodes(done).filter(node => ['status', 'alert'].includes(node.props?.role)).length, 0);
  assert.equal(h.errors.length, 0);
});

test('export failure is logged and visibly reported and controls recover', async () => {
  const h = harness();
  const exporting = button(h.render(), 'Export All').props.onClick();
  h.reject('Permission denied; partial export in C:\\Exports\\partial');
  await exporting;
  const done = h.render();
  assert.equal(button(done, 'Export All').props.disabled, false);
  assert.equal(button(done, 'Clear All').props.disabled, false);
  const alert = nodes(done).find(node => node.props?.role === 'alert');
  assert.ok(text(alert).includes('Permission denied'));
  assert.ok(text(alert).includes('C:\\Exports\\partial'));
  assert.equal(h.errors.length, 1);
});
